//! HTTP client for calling the Burd API

use serde_json::Value;

const API_BASE: &str = "http://127.0.0.1:19840";

pub struct BurdApiClient {
    client: reqwest::blocking::Client,
}

impl BurdApiClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Check if the Burd API is available
    pub fn is_available(&self) -> bool {
        self.client
            .get(&format!("{}/status", API_BASE))
            .send()
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub fn get(&self, path: &str) -> Result<String, String> {
        let response = self
            .client
            .get(&format!("{}{}", API_BASE, path))
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        self.handle_response(response)
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<String, String> {
        let response = self
            .client
            .post(&format!("{}{}", API_BASE, path))
            .json(body)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        self.handle_response(response)
    }

    pub fn put(&self, path: &str, body: &Value) -> Result<String, String> {
        let response = self
            .client
            .put(&format!("{}{}", API_BASE, path))
            .json(body)
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        self.handle_response(response)
    }

    pub fn delete(&self, path: &str) -> Result<String, String> {
        let response = self
            .client
            .delete(&format!("{}{}", API_BASE, path))
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        self.handle_response(response)
    }

    fn handle_response(&self, response: reqwest::blocking::Response) -> Result<String, String> {
        let status = response.status();
        let body = response
            .text()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if status.is_success() {
            // Parse the ApiResponse and extract data
            if let Ok(api_response) = serde_json::from_str::<Value>(&body) {
                if api_response.get("success").and_then(|v| v.as_bool()) == Some(true) {
                    if let Some(data) = api_response.get("data") {
                        return Ok(serde_json::to_string_pretty(data).unwrap_or(body));
                    }
                    // Success with no data
                    return Ok("Operation completed successfully".to_string());
                }
                if let Some(error) = api_response.get("error").and_then(|v| v.as_str()) {
                    return Err(error.to_string());
                }
            }
            Ok(body)
        } else {
            // Try to parse error from response body
            if let Ok(api_response) = serde_json::from_str::<Value>(&body) {
                if let Some(error) = api_response.get("error").and_then(|v| v.as_str()) {
                    return Err(error.to_string());
                }
            }
            Err(format!("API error ({}): {}", status, body))
        }
    }
}

impl Default for BurdApiClient {
    fn default() -> Self {
        Self::new()
    }
}
