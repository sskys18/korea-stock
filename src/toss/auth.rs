use std::path::PathBuf;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::toss::config::TossConfig;
use crate::toss::error::{Result, TossError};

#[derive(Serialize, Deserialize, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
    /// 발급 base+client_id 식별자 — 다중 클라이언트 토큰 혼용 방지.
    #[serde(default)]
    scope: String,
}

impl CachedToken {
    /// 만료 1시간 전이면 아직 유효. KIS와 동일 정책.
    fn is_fresh(&self) -> bool {
        self.expires_at > Utc::now() + ChronoDuration::hours(1)
    }
}

/// OAuth2 토큰 발급·캐싱 담당.
///
/// 캐싱 전략은 KIS `Auth`와 동일(메모리 → 파일 → 신규 발급, Mutex single-flight,
/// scope 격리, atomic 0600 쓰기). 차이는 발급 요청이 `application/x-www-form-urlencoded`
/// Client Credentials Grant이고, 발급 실패가 OAuth2 표준 에러라는 점이다.
///
/// **single-flight가 KIS보다 더 중요하다**: 토스는 client당 유효 토큰 1개이며 재발급 시
/// 이전 토큰을 즉시 무효화한다. 동시 발급이 일어나면 한쪽이 무효 토큰을 들고 호출 → 401.
/// `Mutex`로 직렬화하여 1회만 발급한다.
pub(crate) struct Auth {
    client_id: String,
    client_secret: String,
    base_url: String,
    cache_path: Option<PathBuf>,
    http: reqwest::Client,
    state: Mutex<Option<CachedToken>>,
}

impl Auth {
    pub fn new(config: &TossConfig, http: reqwest::Client) -> Self {
        Self {
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            base_url: config.base_url.clone(),
            cache_path: config.token_cache_path.clone(),
            http,
            state: Mutex::new(None),
        }
    }

    /// 유효한 access token 반환. 메모리 → 파일 캐시 → 신규 발급 순.
    /// `Mutex`로 동시 호출 시 1회만 발급.
    pub async fn token(&self) -> Result<String> {
        let mut state = self.state.lock().await;

        if let Some(t) = state.as_ref() {
            if t.is_fresh() {
                return Ok(t.access_token.clone());
            }
        }
        // 메모리 토큰이 없거나 stale이면 파일 캐시 확인 — 다른 프로세스가 갱신해 둔
        // 신선한 토큰을 stale 메모리 토큰 때문에 놓치지 않도록.
        if let Some(t) = self.load_cache().await {
            if t.is_fresh() {
                let token = t.access_token.clone();
                *state = Some(t);
                return Ok(token);
            }
        }
        let fresh = self.issue().await?;
        self.save_cache(&fresh).await;
        let token = fresh.access_token.clone();
        *state = Some(fresh);
        Ok(token)
    }

    /// 토큰 캐시 스코프 — `{base_url}|{client_id}`. base·client별 격리.
    fn scope(&self) -> String {
        format!("{}|{}", self.base_url, self.client_id)
    }

    /// `POST /oauth2/token` (Client Credentials, form-urlencoded).
    async fn issue(&self) -> Result<CachedToken> {
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
        }
        let resp = self
            .http
            .post(format!("{}/oauth2/token", self.base_url))
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
            ])
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            // OAuth2 표준 에러 본문 파싱 시도 — BFF envelope 아님.
            #[derive(Deserialize)]
            struct OAuth2Err {
                error: String,
                error_description: Option<String>,
            }
            let body = resp.text().await.unwrap_or_default();
            if let Ok(e) = serde_json::from_str::<OAuth2Err>(&body) {
                return Err(TossError::OAuth2 {
                    error: e.error,
                    description: e.error_description,
                });
            }
            return Err(TossError::OAuth2 {
                error: format!("http_{}", status.as_u16()),
                description: if body.is_empty() { None } else { Some(body) },
            });
        }
        let tr: TokenResponse = resp.json().await?;
        Ok(CachedToken {
            access_token: tr.access_token,
            expires_at: Utc::now() + ChronoDuration::seconds(tr.expires_in),
            scope: self.scope(),
        })
    }

    async fn load_cache(&self) -> Option<CachedToken> {
        let path = self.cache_path.as_ref()?;
        let data = tokio::fs::read(path).await.ok()?;
        let token: CachedToken = serde_json::from_slice(&data).ok()?;
        // 다른 base·client로 발급된 토큰이면 무시 — 캐시 파일 공유 시 혼용 방지.
        if token.scope != self.scope() {
            return None;
        }
        Some(token)
    }

    /// 임시파일 → chmod 0600 → atomic rename. 실패 시 warn만 (치명적 아님).
    async fn save_cache(&self, token: &CachedToken) {
        let Some(path) = self.cache_path.as_ref() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let tmp = path.with_extension("tmp");
        let json = match serde_json::to_vec_pretty(token) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("toss token cache serialize failed: {e}");
                return;
            }
        };
        if let Err(e) = tokio::fs::write(&tmp, &json).await {
            tracing::warn!("toss token cache write failed: {e}");
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perm = std::fs::Permissions::from_mode(0o600);
            if let Err(e) = tokio::fs::set_permissions(&tmp, perm).await {
                tracing::warn!("toss token cache chmod failed: {e}");
            }
        }
        if let Err(e) = tokio::fs::rename(&tmp, path).await {
            tracing::warn!("toss token cache rename failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_token_within_ttl() {
        let t = CachedToken {
            access_token: "x".into(),
            expires_at: Utc::now() + ChronoDuration::hours(5),
            scope: String::new(),
        };
        assert!(t.is_fresh());
    }

    #[test]
    fn stale_token_near_expiry() {
        let t = CachedToken {
            access_token: "x".into(),
            expires_at: Utc::now() + ChronoDuration::minutes(30),
            scope: String::new(),
        };
        assert!(!t.is_fresh(), "만료 1시간 이내는 stale");
    }
}
