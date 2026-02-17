//! Mailpit notification service
//!
//! Connects to Mailpit's WebSocket API to receive real-time email notifications.
//! Emits events to the frontend when new emails arrive.

use crate::commands::AppState;
use crate::config::ServiceType;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_tungstenite::connect_async;

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
}

impl Default for MailNotifierState {
    fn default() -> Self {
        Self {
            running: AtomicBool::new(false),
        }
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
