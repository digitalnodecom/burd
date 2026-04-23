//! Mailpit API handlers
//!
//! Thin HTTP wrapper around the existing Tauri `commands::mail` module so the
//! MCP and external clients can read/manage captured mail without going through
//! the desktop IPC layer.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::api::{state::ApiState, types::ApiResponse};
use crate::commands::mail::{MailMessageDetail, MailMessageList, SmtpConfig};
use crate::commands::AppState;
use crate::config::ServiceType;

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
});

struct MailpitPorts {
    http_port: u16,
    smtp_port: u16,
}

/// Locate the (single) Mailpit instance and confirm it's running.
///
/// Returns an error string suitable for a 503 response when Mailpit is either
/// not configured at all or configured but not running — the caller can't do
/// anything useful either way.
fn get_mailpit_ports(state: &Arc<AppState>) -> Result<MailpitPorts, String> {
    let config_store = state
        .config_store
        .lock()
        .map_err(|_| "Failed to lock config")?;
    let config = config_store.load().map_err(|e| e.to_string())?;

    let mailpit = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Mailpit)
        .ok_or("No Mailpit instance configured")?;

    let process_manager = state
        .process_manager
        .lock()
        .map_err(|_| "Failed to lock process manager")?;
    if !process_manager.is_running(&mailpit.id) {
        return Err("Mailpit is not running".to_string());
    }

    // `smtp_port` is historically stored as a string; accept either shape.
    let smtp_port = mailpit
        .config
        .get("smtp_port")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<u16>().ok())
                .or_else(|| v.as_u64().map(|n| n as u16))
        })
        .unwrap_or(1025);

    Ok(MailpitPorts {
        http_port: mailpit.port,
        smtp_port,
    })
}

fn unavailable(msg: impl Into<String>) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiResponse::<()>::err(msg.into())),
    )
        .into_response()
}

fn upstream_err(msg: impl Into<String>) -> Response {
    (
        StatusCode::BAD_GATEWAY,
        Json(ApiResponse::<()>::err(msg.into())),
    )
        .into_response()
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub start: Option<u32>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub search: Option<String>,
}

#[derive(Deserialize)]
pub struct MarkReadRequest {
    pub ids: Vec<String>,
    pub read: bool,
}

#[derive(Serialize)]
pub struct UnreadCount {
    pub unread: u32,
}

/// GET /mail/config - SMTP + HTTP ports for Mailpit
pub async fn config(State(state): State<ApiState>) -> Response {
    match get_mailpit_ports(&state.inner) {
        Ok(p) => Json(ApiResponse::ok(SmtpConfig {
            host: "127.0.0.1".to_string(),
            port: p.smtp_port,
            http_port: p.http_port,
        }))
        .into_response(),
        Err(e) => unavailable(e),
    }
}

/// GET /mail - list captured messages (with optional search/pagination)
pub async fn list(State(state): State<ApiState>, Query(q): Query<ListQuery>) -> Response {
    let port = match get_mailpit_ports(&state.inner) {
        Ok(p) => p.http_port,
        Err(e) => return unavailable(e),
    };

    let has_search = q.search.as_ref().is_some_and(|s| !s.is_empty());
    let base = if has_search { "search" } else { "messages" };
    let mut url = format!("http://127.0.0.1:{}/api/v1/{}", port, base);

    let mut params = Vec::new();
    if let Some(s) = q.start {
        params.push(format!("start={}", s));
    }
    if let Some(l) = q.limit {
        params.push(format!("limit={}", l));
    }
    if let Some(query) = q.search {
        if !query.is_empty() {
            params.push(format!("query={}", urlencoding::encode(&query)));
        }
    }
    if !params.is_empty() {
        url = format!("{}?{}", url, params.join("&"));
    }

    let resp = match HTTP_CLIENT.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return upstream_err(format!("Failed to fetch emails: {}", e)),
    };
    if !resp.status().is_success() {
        return upstream_err(format!("Mailpit API error: {}", resp.status()));
    }
    match resp.json::<MailMessageList>().await {
        Ok(data) => Json(ApiResponse::ok(data)).into_response(),
        Err(e) => upstream_err(format!("Failed to parse response: {}", e)),
    }
}

/// GET /mail/:id - single message detail
pub async fn get(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    let port = match get_mailpit_ports(&state.inner) {
        Ok(p) => p.http_port,
        Err(e) => return unavailable(e),
    };

    let url = format!("http://127.0.0.1:{}/api/v1/message/{}", port, id);
    let resp = match HTTP_CLIENT.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return upstream_err(format!("Failed to fetch email: {}", e)),
    };
    if !resp.status().is_success() {
        return upstream_err(format!("Mailpit API error: {}", resp.status()));
    }
    match resp.json::<MailMessageDetail>().await {
        Ok(data) => Json(ApiResponse::ok(data)).into_response(),
        Err(e) => upstream_err(format!("Failed to parse response: {}", e)),
    }
}

/// DELETE /mail/:id - delete a single message
pub async fn delete_one(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    let port = match get_mailpit_ports(&state.inner) {
        Ok(p) => p.http_port,
        Err(e) => return unavailable(e),
    };

    #[derive(Serialize)]
    struct DeleteReq {
        #[serde(rename = "IDs")]
        ids: Vec<String>,
    }

    let url = format!("http://127.0.0.1:{}/api/v1/messages", port);
    let resp = match HTTP_CLIENT
        .delete(&url)
        .json(&DeleteReq { ids: vec![id] })
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return upstream_err(format!("Failed to delete email: {}", e)),
    };
    if !resp.status().is_success() {
        return upstream_err(format!("Mailpit API error: {}", resp.status()));
    }
    Json(ApiResponse::<()>::success()).into_response()
}

/// DELETE /mail - delete all messages
pub async fn delete_all(State(state): State<ApiState>) -> Response {
    let port = match get_mailpit_ports(&state.inner) {
        Ok(p) => p.http_port,
        Err(e) => return unavailable(e),
    };

    let url = format!("http://127.0.0.1:{}/api/v1/messages", port);
    let resp = match HTTP_CLIENT.delete(&url).send().await {
        Ok(r) => r,
        Err(e) => return upstream_err(format!("Failed to delete all emails: {}", e)),
    };
    if !resp.status().is_success() {
        return upstream_err(format!("Mailpit API error: {}", resp.status()));
    }
    Json(ApiResponse::<()>::success()).into_response()
}

/// PUT /mail/read - mark a set of messages read/unread
pub async fn mark_read(
    State(state): State<ApiState>,
    Json(req): Json<MarkReadRequest>,
) -> Response {
    let port = match get_mailpit_ports(&state.inner) {
        Ok(p) => p.http_port,
        Err(e) => return unavailable(e),
    };

    #[derive(Serialize)]
    struct ReadReq {
        #[serde(rename = "IDs")]
        ids: Vec<String>,
        #[serde(rename = "Read")]
        read: bool,
    }

    let url = format!("http://127.0.0.1:{}/api/v1/messages", port);
    let resp = match HTTP_CLIENT
        .put(&url)
        .json(&ReadReq {
            ids: req.ids,
            read: req.read,
        })
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return upstream_err(format!("Failed to update read status: {}", e)),
    };
    if !resp.status().is_success() {
        return upstream_err(format!("Mailpit API error: {}", resp.status()));
    }
    Json(ApiResponse::<()>::success()).into_response()
}

/// GET /mail/unread-count
pub async fn unread_count(State(state): State<ApiState>) -> Response {
    let port = match get_mailpit_ports(&state.inner) {
        Ok(p) => p.http_port,
        Err(e) => return unavailable(e),
    };

    let url = format!("http://127.0.0.1:{}/api/v1/messages?limit=0", port);
    let resp = match HTTP_CLIENT.get(&url).send().await {
        Ok(r) => r,
        Err(e) => return upstream_err(format!("Failed to fetch unread count: {}", e)),
    };
    if !resp.status().is_success() {
        return upstream_err(format!("Mailpit API error: {}", resp.status()));
    }
    match resp.json::<MailMessageList>().await {
        Ok(list) => Json(ApiResponse::ok(UnreadCount { unread: list.unread })).into_response(),
        Err(e) => upstream_err(format!("Failed to parse response: {}", e)),
    }
}
