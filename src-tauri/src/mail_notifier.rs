//! Mailpit notification service
//!
//! Connects to Mailpit's WebSocket API to receive real-time email notifications.
//! Emits events to the frontend when new emails arrive.

use crate::commands::AppState;
use crate::config::ServiceType;
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use tokio_tungstenite::connect_async;

static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .expect("Failed to create HTTP client")
});

#[derive(Debug, Deserialize)]
struct UnreadCountResponse {
    unread: u32,
}

/// Fetch current unread count from Mailpit and update the app badge.
/// Safe to call from anywhere; failures are silent.
async fn refresh_badge(app_handle: &AppHandle, port: u16) -> Option<u32> {
    let url = format!("http://127.0.0.1:{}/api/v1/messages?limit=0", port);
    let count = HTTP_CLIENT
        .get(&url)
        .send()
        .await
        .ok()?
        .json::<UnreadCountResponse>()
        .await
        .ok()?
        .unread;
    set_badge(app_handle, count);
    Some(count)
}

fn set_badge(app_handle: &AppHandle, count: u32) {
    let value = if count == 0 { None } else { Some(count as i64) };
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_badge_count(value);
    }
}

#[tauri::command]
pub async fn sync_mail_badge(app_handle: AppHandle) -> Result<u32, String> {
    let state = app_handle.state::<AppState>();
    let port = match get_mailpit_port(&state) {
        Some(p) => p,
        None => {
            set_badge(&app_handle, 0);
            return Ok(0);
        }
    };
    Ok(refresh_badge(&app_handle, port).await.unwrap_or(0))
}

/// Payload emitted when a new email arrives
#[derive(Debug, Clone, Serialize)]
pub struct NewEmailPayload {
    pub from_name: String,
    pub from_address: String,
    pub subject: String,
    pub id: String,
}

/// Mailpit WebSocket message types
#[derive(Debug, Deserialize)]
#[serde(tag = "Type")]
enum MailpitEvent {
    #[serde(rename = "new")]
    New {
        #[serde(rename = "Data")]
        data: MailpitNewEmail,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct MailpitNewEmail {
    ID: String,
    #[serde(default)]
    From: Option<MailpitAddress>,
    #[serde(default)]
    Subject: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct MailpitAddress {
    Name: String,
    Address: String,
}

/// Shared state for mail notifier
pub struct MailNotifierState {
    running: AtomicBool,
    /// Most recent email id surfaced via OS notification, with the moment it was shown.
    /// Consumed when the user brings the app to focus shortly after.
    pending_focus: Mutex<Option<(String, Instant)>>,
}

impl Default for MailNotifierState {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
            pending_focus: Mutex::new(None),
        }
    }
}

/// How long after a notification a focus event still counts as "the user clicked it".
const FOCUS_CLAIM_TTL: Duration = Duration::from_secs(15);

/// Called from the main window's focus handler. If a pending notification is
/// recent enough, emit `open-email` so the frontend navigates to it.
pub fn handle_window_focus(app_handle: &AppHandle) {
    let state = app_handle.state::<MailNotifierState>();
    let mut guard = match state.pending_focus.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    let Some((id, when)) = guard.take() else {
        return;
    };
    if when.elapsed() <= FOCUS_CLAIM_TTL {
        let _ = app_handle.emit("open-email", id);
    }
}

/// Get the Mailpit HTTP port from config
fn get_mailpit_port(state: &State<'_, AppState>) -> Option<u16> {
    let config_store = state.config_store.lock().ok()?;
    let config = config_store.load().ok()?;

    let mailpit = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Mailpit)?;

    // Check if running
    let process_manager = state.process_manager.lock().ok()?;
    if !process_manager.is_running(&mailpit.id) {
        return None;
    }

    Some(mailpit.port)
}

/// Start the mail notifier WebSocket listener
pub fn start_mail_notifier(app_handle: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        let notifier_state = app_handle.state::<MailNotifierState>();

        // Check if already running
        if notifier_state.running.swap(true, Ordering::SeqCst) {
            return;
        }

        loop {
            // Get Mailpit port
            let port = match get_mailpit_port(&state) {
                Some(p) => p,
                None => {
                    // Mailpit not running, wait and retry
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                    continue;
                }
            };

            let ws_url = format!("ws://127.0.0.1:{}/api/events", port);

            if let Ok((ws_stream, _)) = connect_async(&ws_url).await {
                let (_, mut read) = ws_stream.split();

                while let Some(msg_result) = read.next().await {
                    match msg_result {
                        Ok(msg) => {
                            if let Ok(text) = msg.to_text() {
                                if let Ok(MailpitEvent::New { data: email }) =
                                    serde_json::from_str::<MailpitEvent>(text)
                                {
                                    let payload = NewEmailPayload {
                                        from_name: email
                                            .From
                                            .as_ref()
                                            .map(|f| f.Name.clone())
                                            .unwrap_or_default(),
                                        from_address: email
                                            .From
                                            .as_ref()
                                            .map(|f| f.Address.clone())
                                            .unwrap_or_else(|| "Unknown".to_string()),
                                        subject: email
                                            .Subject
                                            .unwrap_or_else(|| "(No subject)".to_string()),
                                        id: email.ID,
                                    };

                                    // Emit event to frontend
                                    let _ = app_handle.emit("new-email", payload.clone());

                                    // Show OS notification
                                    let title = if payload.from_name.is_empty() {
                                        format!("New mail from {}", payload.from_address)
                                    } else {
                                        format!("New mail from {}", payload.from_name)
                                    };
                                    let _ = app_handle
                                        .notification()
                                        .builder()
                                        .title(title)
                                        .body(&payload.subject)
                                        .show();

                                    // If the user is not already in the app, remember
                                    // this email so we can navigate to it on focus.
                                    let window_focused = app_handle
                                        .get_webview_window("main")
                                        .and_then(|w| w.is_focused().ok())
                                        .unwrap_or(false);
                                    if !window_focused {
                                        let notifier_state =
                                            app_handle.state::<MailNotifierState>();
                                        let mut guard =
                                            notifier_state.pending_focus.lock().unwrap();
                                        *guard = Some((payload.id.clone(), Instant::now()));
                                    }

                                    // Refresh the dock/taskbar badge from Mailpit
                                    let _ = refresh_badge(&app_handle, port).await;
                                }
                            }
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }
            }

            // Wait before reconnecting
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    });
}
