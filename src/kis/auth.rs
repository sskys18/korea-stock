use std::path::PathBuf;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::kis::config::KisConfig;
use crate::kis::error::{KisError, Result};

#[derive(Serialize, Deserialize, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
    /// 발급 환경+앱키 식별자 — 실전/모의·다중 앱키 토큰 혼용 방지.
    #[serde(default)]
    scope: String,
}

impl CachedToken {
    /// 만료 1시간 전이면 아직 유효.
    fn is_fresh(&self) -> bool {
        self.expires_at > Utc::now() + ChronoDuration::hours(1)
    }
}

/// 토큰 발급·캐싱·hashkey 발급 담당.
pub(crate) struct Auth {
    app_key: String,
    app_secret: String,
    rest_base: String,
    cache_path: Option<PathBuf>,
    http: reqwest::Client,
    state: Mutex<Option<CachedToken>>,
}

impl Auth {
    pub fn new(config: &KisConfig, http: reqwest::Client) -> Self {
        Self {
            app_key: config.app_key.clone(),
            app_secret: config.app_secret.clone(),
            rest_base: config.environment.rest_base().to_string(),
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
        // 메모리 토큰이 없거나 stale이면 파일 캐시 확인 — 다른 프로세스가
        // 갱신해 둔 신선한 토큰을 stale 메모리 토큰 때문에 놓치지 않도록.
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

    /// 토큰 캐시 스코프 — `{rest_base}|{app_key}`. 환경·앱키별 격리.
    fn scope(&self) -> String {
        format!("{}|{}", self.rest_base, self.app_key)
    }

    async fn issue(&self) -> Result<CachedToken> {
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
        }
        let body = serde_json::json!({
            "grant_type": "client_credentials",
            "appkey": self.app_key,
            "appsecret": self.app_secret,
        });
        let resp = self
            .http
            .post(format!("{}/oauth2/tokenP", self.rest_base))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(KisError::Auth(format!("token issue failed: {txt}")));
        }
        let tr: TokenResponse = resp.json().await?;
        Ok(CachedToken {
            access_token: tr.access_token,
            expires_at: Utc::now() + ChronoDuration::seconds(tr.expires_in),
            scope: self.scope(),
        })
    }

    /// 주문 body의 hashkey 발급. `use_hashkey=true`일 때만 호출.
    pub async fn hashkey(&self, body: &serde_json::Value) -> Result<String> {
        #[derive(Deserialize)]
        struct HashResponse {
            #[serde(rename = "HASH")]
            hash: String,
        }
        let resp = self
            .http
            .post(format!("{}/uapi/hashkey", self.rest_base))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(KisError::Auth(format!("hashkey failed: {txt}")));
        }
        let h: HashResponse = resp.json().await?;
        Ok(h.hash)
    }

    async fn load_cache(&self) -> Option<CachedToken> {
        let path = self.cache_path.as_ref()?;
        let data = tokio::fs::read(path).await.ok()?;
        let token: CachedToken = serde_json::from_slice(&data).ok()?;
        // 다른 환경·앱키로 발급된 토큰이면 무시 — 캐시 파일 공유 시 혼용 방지.
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
                tracing::warn!("token cache serialize failed: {e}");
                return;
            }
        };
        if let Err(e) = tokio::fs::write(&tmp, &json).await {
            tracing::warn!("token cache write failed: {e}");
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perm = std::fs::Permissions::from_mode(0o600);
            if let Err(e) = tokio::fs::set_permissions(&tmp, perm).await {
                tracing::warn!("token cache chmod failed: {e}");
            }
        }
        if let Err(e) = tokio::fs::rename(&tmp, path).await {
            tracing::warn!("token cache rename failed: {e}");
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
