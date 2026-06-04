//! WebSocket 접속키(approval_key) 발급.
//!
//! `POST /oauth2/Approval` — access token과 별개의 인증 수단.
//! 요청 Body 필드명이 `secretkey`임에 주의 (REST 토큰의 `appsecret`과 다름,
//! docs/kis-api/realtime.md §A-2).

use serde::Deserialize;

use crate::domestic::kis::config::KisConfig;
use crate::domestic::kis::error::{KisError, Result};

/// approval_key를 발급한다. 만료/캐싱은 하지 않음 — WS 연결을 열 때마다
/// (최초·재연결 공통) 새로 발급한다 (D7). 발급 빈도가 낮아 토큰버킷 불필요.
pub(crate) async fn issue_approval_key(
    http: &reqwest::Client,
    config: &KisConfig,
) -> Result<String> {
    #[derive(Default, Deserialize)]
    #[serde(default)]
    struct ApprovalResponse {
        approval_key: String,
    }

    let url = format!("{}/oauth2/Approval", config.environment.rest_base());
    let body = serde_json::json!({
        "grant_type": "client_credentials",
        "appkey": &config.app_key,
        "secretkey": &config.app_secret,
    });

    let resp = http
        .post(&url)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        return Err(KisError::Auth(format!(
            "approval_key issue failed (http {status}): {txt}"
        )));
    }

    let parsed: ApprovalResponse = resp.json().await?;
    if parsed.approval_key.is_empty() {
        return Err(KisError::Auth("approval_key empty in response".into()));
    }
    Ok(parsed.approval_key)
}
