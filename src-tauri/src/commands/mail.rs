use crate::commands::AppState;
use crate::config::ServiceType;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tauri::State;

// ============================================================================
// Helper: Fix double-encoded UTF-8 strings
// ============================================================================
//
// Mailpit sometimes returns double-encoded UTF-8 when emails don't specify
// charset properly. This happens when UTF-8 bytes are misinterpreted as
// Latin-1 and then re-encoded to UTF-8.
//
// Example: "Тест" (Cyrillic) as UTF-8 bytes D0 A2 D0 B5 D1 81 D1 82
// gets misread as Latin-1 characters "Тест" and re-encoded to UTF-8 as
// C3 90 C2 A2 C3 90 C2 B5 C3 91 C2 81 C3 91 C2 82

fn fix_double_encoded_utf8(s: &str) -> String {
    // Try to decode as if the string was Latin-1 encoded UTF-8 bytes
    // Latin-1 is a 1:1 mapping of bytes 0x00-0xFF to Unicode U+0000-U+00FF
    let bytes: Vec<u8> = s.chars()
        .filter_map(|c| {
            let code = c as u32;
            if code <= 0xFF {
                Some(code as u8)
            } else {
                None // Character outside Latin-1 range, can't be double-encoded
            }
        })
        .collect();

    // If we lost characters, the string wasn't double-encoded Latin-1
    if bytes.len() != s.chars().count() {
        return s.to_string();
    }

    // Try to parse those bytes as UTF-8
    match String::from_utf8(bytes) {
        Ok(decoded) => {
            // Verify it's actually different and looks like real text
            // (contains non-ASCII chars that weren't there before as Ã/Â patterns)
            if decoded != s && !decoded.contains('\u{FFFD}') {
                decoded
            } else {
                s.to_string()
            }
        }
        Err(_) => s.to_string(), // Not valid UTF-8, keep original
    }
}

fn fix_mail_encoding(msg: &mut MailMessageSummary) {
    msg.subject = fix_double_encoded_utf8(&msg.subject);
    msg.snippet = fix_double_encoded_utf8(&msg.snippet);
    msg.from.name = fix_double_encoded_utf8(&msg.from.name);
    for to in &mut msg.to {
        to.name = fix_double_encoded_utf8(&to.name);
    }
}

fn fix_mail_detail_encoding(msg: &mut MailMessageDetail) {
    msg.subject = fix_double_encoded_utf8(&msg.subject);
    msg.text = fix_double_encoded_utf8(&msg.text);
    msg.from.name = fix_double_encoded_utf8(&msg.from.name);
    for to in &mut msg.to {
        to.name = fix_double_encoded_utf8(&to.name);
    }
    for cc in &mut msg.cc {
        cc.name = fix_double_encoded_utf8(&cc.name);
    }
}

// Shared HTTP client with timeout to prevent hanging requests
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("Failed to create HTTP client")
});

// ============================================================================
// Types for Mailpit API responses
// ============================================================================

// Use `alias` instead of `rename` so that:
// - Deserialization from Mailpit API accepts PascalCase fields
// - Serialization to frontend uses snake_case field names

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MailAddress {
    #[serde(alias = "Name")]
    pub name: String,
    #[serde(alias = "Address")]
    pub address: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MailAttachment {
    #[serde(alias = "PartID")]
    pub part_id: String,
    #[serde(alias = "FileName")]
    pub file_name: String,
    #[serde(alias = "ContentType")]
    pub content_type: String,
    #[serde(alias = "Size")]
    pub size: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MailMessageSummary {
    #[serde(alias = "ID")]
    pub id: String,
    #[serde(alias = "MessageID")]
    pub message_id: String,
    #[serde(alias = "From")]
    pub from: MailAddress,
    #[serde(alias = "To")]
    pub to: Vec<MailAddress>,
    #[serde(alias = "Subject")]
    pub subject: String,
    #[serde(alias = "Created")]
    pub created: String,
    #[serde(alias = "Size")]
    pub size: u64,
    #[serde(alias = "Read")]
    pub read: bool,
    #[serde(alias = "Snippet")]
    pub snippet: String,
    #[serde(alias = "Attachments")]
    pub attachments: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MailMessageList {
    pub total: u32,
    pub unread: u32,
    pub count: u32,
    pub start: u32,
    pub messages: Vec<MailMessageSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MailMessageDetail {
    #[serde(alias = "ID")]
    pub id: String,
    #[serde(alias = "MessageID")]
    pub message_id: String,
    #[serde(alias = "From")]
    pub from: MailAddress,
    #[serde(alias = "To")]
    pub to: Vec<MailAddress>,
    #[serde(alias = "Cc", default)]
    pub cc: Vec<MailAddress>,
    #[serde(alias = "Bcc", default)]
    pub bcc: Vec<MailAddress>,
    #[serde(alias = "ReplyTo", default)]
    pub reply_to: Vec<MailAddress>,
    #[serde(alias = "Subject")]
    pub subject: String,
    #[serde(alias = "Date")]
    pub date: String,
    #[serde(alias = "Size")]
    pub size: u64,
    #[serde(alias = "HTML", default)]
    pub html: String,
    #[serde(alias = "Text", default)]
    pub text: String,
    #[serde(alias = "Attachments", default)]
    pub attachments: Vec<MailAttachment>,
}

#[derive(Debug, Serialize)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub http_port: u16,
}

// ============================================================================
// Helper: get Mailpit instance port
// ============================================================================

fn get_mailpit_port(state: &State<'_, AppState>) -> Result<u16, String> {
    let config_store = state.config_store.lock().map_err(|_| "Failed to lock config")?;
    let config = config_store.load().map_err(|e| e.to_string())?;

    // Find Mailpit instance
    let mailpit = config.instances.iter()
        .find(|i| i.service_type == ServiceType::Mailpit)
        .ok_or("No Mailpit instance found")?;

    // Check if running via ProcessManager
    let process_manager = state.process_manager.lock().map_err(|_| "Failed to lock process manager")?;
    if !process_manager.is_running(&mailpit.id) {
        return Err("Mailpit is not running".to_string());
    }

    Ok(mailpit.port)
}

fn get_mailpit_smtp_port(state: &State<'_, AppState>) -> Result<u16, String> {
    let config_store = state.config_store.lock().map_err(|_| "Failed to lock config")?;
    let config = config_store.load().map_err(|e| e.to_string())?;

    let mailpit = config.instances.iter()
        .find(|i| i.service_type == ServiceType::Mailpit)
        .ok_or("No Mailpit instance found")?;

    let smtp_port = mailpit.config.get("smtp_port")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(1025);

    Ok(smtp_port)
}

// ============================================================================
// Commands
// ============================================================================

#[tauri::command]
pub async fn get_mailpit_config(state: State<'_, AppState>) -> Result<SmtpConfig, String> {
    let http_port = get_mailpit_port(&state)?;
    let smtp_port = get_mailpit_smtp_port(&state)?;

    Ok(SmtpConfig {
        host: "127.0.0.1".to_string(),
        port: smtp_port,
        http_port,
    })
}

#[tauri::command]
pub async fn list_emails(
    state: State<'_, AppState>,
    start: Option<u32>,
    limit: Option<u32>,
    search: Option<String>,
) -> Result<MailMessageList, String> {
    let port = get_mailpit_port(&state)?;

    let client = &*HTTP_CLIENT;

    // Use /search endpoint when searching, /messages otherwise
    let has_search = search.as_ref().map_or(false, |q| !q.is_empty());
    let base_path = if has_search { "search" } else { "messages" };
    let mut url = format!("http://127.0.0.1:{}/api/v1/{}", port, base_path);

    // Build query params
    let mut params = Vec::new();
    if let Some(s) = start {
        params.push(format!("start={}", s));
    }
    if let Some(l) = limit {
        params.push(format!("limit={}", l));
    }
    if let Some(q) = search {
        if !q.is_empty() {
            params.push(format!("query={}", urlencoding::encode(&q)));
        }
    }
    if !params.is_empty() {
        url = format!("{}?{}", url, params.join("&"));
    }

    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch emails: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Mailpit API error: {}", response.status()));
    }

    let mut result: MailMessageList = response.json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Fix double-encoded UTF-8 in email fields
    for msg in &mut result.messages {
        fix_mail_encoding(msg);
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_email(
    state: State<'_, AppState>,
    message_id: String,
) -> Result<MailMessageDetail, String> {
    let port = get_mailpit_port(&state)?;

    let client = &*HTTP_CLIENT;
    let url = format!("http://127.0.0.1:{}/api/v1/message/{}", port, message_id);

    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch email: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Mailpit API error: {}", response.status()));
    }

    let mut result: MailMessageDetail = response.json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    // Fix double-encoded UTF-8 in email fields
    fix_mail_detail_encoding(&mut result);

    Ok(result)
}

#[tauri::command]
pub async fn delete_emails(
    state: State<'_, AppState>,
    message_ids: Vec<String>,
) -> Result<(), String> {
    let port = get_mailpit_port(&state)?;

    let client = &*HTTP_CLIENT;
    let url = format!("http://127.0.0.1:{}/api/v1/messages", port);

    #[derive(Serialize)]
    struct DeleteRequest {
        #[serde(rename = "IDs")]
        ids: Vec<String>,
    }

    let response = client.delete(&url)
        .json(&DeleteRequest { ids: message_ids })
        .send()
        .await
        .map_err(|e| format!("Failed to delete emails: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Mailpit API error: {}", response.status()));
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_all_emails(state: State<'_, AppState>) -> Result<(), String> {
    let port = get_mailpit_port(&state)?;

    let client = &*HTTP_CLIENT;
    let url = format!("http://127.0.0.1:{}/api/v1/messages", port);

    let response = client.delete(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to delete all emails: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Mailpit API error: {}", response.status()));
    }

    Ok(())
}

#[tauri::command]
pub async fn mark_emails_read(
    state: State<'_, AppState>,
    message_ids: Vec<String>,
    read: bool,
) -> Result<(), String> {
    let port = get_mailpit_port(&state)?;

    let client = &*HTTP_CLIENT;
    let url = format!("http://127.0.0.1:{}/api/v1/messages", port);

    #[derive(Serialize)]
    struct ReadRequest {
        #[serde(rename = "IDs")]
        ids: Vec<String>,
        #[serde(rename = "Read")]
        read: bool,
    }

    let response = client.put(&url)
        .json(&ReadRequest { ids: message_ids, read })
        .send()
        .await
        .map_err(|e| format!("Failed to update read status: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Mailpit API error: {}", response.status()));
    }

    Ok(())
}

#[tauri::command]
pub async fn get_unread_count(state: State<'_, AppState>) -> Result<u32, String> {
    let port = get_mailpit_port(&state)?;

    let client = &*HTTP_CLIENT;
    let url = format!("http://127.0.0.1:{}/api/v1/messages?limit=0", port);

    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch unread count: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Mailpit API error: {}", response.status()));
    }

    let result: MailMessageList = response.json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(result.unread)
}
