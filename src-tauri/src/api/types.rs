//! Shared API request and response types

use serde::{Deserialize, Serialize};

/// Standard API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
}

impl<T: Serialize> ApiResponse<T> {
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

impl ApiResponse<()> {
    pub fn success() -> Self {
        Self {
            success: true,
            data: None,
            error: None,
        }
    }
}

/// Create instance request
#[derive(Deserialize)]
pub struct CreateInstanceRequest {
    pub name: String,
    pub port: u16,
    pub service_type: String,
    pub version: String,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub custom_domain: Option<String>,
}

/// Create domain request
#[derive(Deserialize)]
pub struct CreateDomainRequest {
    pub subdomain: String,
    /// "instance", "port", or "static"
    pub target_type: String,
    /// Instance UUID, port number, or path (depending on target_type)
    pub target_value: String,
    #[serde(default)]
    pub ssl_enabled: bool,
    #[serde(default)]
    pub static_browse: Option<bool>,
}

/// Update domain request
#[derive(Deserialize)]
pub struct UpdateDomainRequest {
    #[serde(default)]
    pub subdomain: Option<String>,
    #[serde(default)]
    pub target_type: Option<String>,
    #[serde(default)]
    pub target_value: Option<String>,
}

/// Toggle SSL request
#[derive(Deserialize)]
pub struct ToggleSslRequest {
    pub ssl_enabled: bool,
}

/// Create database request
#[derive(Deserialize)]
pub struct CreateDatabaseRequest {
    pub name: String,
    /// Optional: target specific DB instance
    #[serde(default)]
    pub instance_id: Option<String>,
}

/// Status response
#[derive(Serialize)]
pub struct StatusResponse {
    pub app_running: bool,
    pub dns_running: bool,
    pub proxy_installed: bool,
    pub tld: String,
    pub instance_count: usize,
    pub running_instances: usize,
}

/// Instance response (simplified for API)
#[derive(Serialize)]
pub struct InstanceResponse {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub service_type: String,
    pub version: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub healthy: Option<bool>,
    pub domain: Option<String>,
    pub domain_enabled: bool,
}

/// Domain response
#[derive(Serialize)]
pub struct DomainResponse {
    pub id: String,
    pub subdomain: String,
    pub full_domain: String,
    pub target_type: String,
    pub target_value: String,
    pub ssl_enabled: bool,
}

/// Service info response
#[derive(Serialize)]
pub struct ServiceResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
}

/// Service versions response
#[derive(Serialize)]
pub struct ServiceVersionsResponse {
    pub service_type: String,
    pub installed: Vec<String>,
}

/// Database info response
#[derive(Serialize)]
pub struct DatabaseResponse {
    pub name: String,
    pub instance_id: String,
    pub instance_name: String,
    pub service_type: String,
}
