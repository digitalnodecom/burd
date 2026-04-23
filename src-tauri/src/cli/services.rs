//! `burd services [versions [TYPE]]` — list services and installed versions.
//!
//! Mirrors the MCP `list_services` and `get_service_versions` tools.

use crate::api_client::BurdApiClient;

fn client() -> Result<BurdApiClient, String> {
    let client = BurdApiClient::new();
    if !client.is_available() {
        return Err(
            "Burd app isn't running. Open Burd or run `burd setup`, then try again.".to_string(),
        );
    }
    Ok(client)
}

pub fn run_services_list() -> Result<(), String> {
    let client = client()?;
    let body = client.get("/services")?;
    println!("{}", body);
    Ok(())
}

/// List installed versions for `service_type`, or for every available service
/// when no type is provided. Passing no argument matches the ergonomics of
/// `brew services list` — cheap discovery without knowing the service id.
pub fn run_service_versions(service_type: Option<String>) -> Result<(), String> {
    let client = client()?;
    match service_type {
        Some(t) => {
            let body = client.get(&format!("/services/{}/versions", t))?;
            println!("{}", body);
        }
        None => {
            let services = client.get("/services")?;
            let parsed: serde_json::Value = serde_json::from_str(&services)
                .map_err(|e| format!("Failed to parse /services response: {}", e))?;
            let arr = parsed
                .as_array()
                .ok_or_else(|| "Expected array from /services".to_string())?;
            for svc in arr {
                let Some(id) = svc.get("id").and_then(|v| v.as_str()) else {
                    continue;
                };
                match client.get(&format!("/services/{}/versions", id)) {
                    Ok(body) => {
                        println!("{}:\n{}\n", id, body);
                    }
                    Err(e) => {
                        println!("{}: error — {}\n", id, e);
                    }
                }
            }
        }
    }
    Ok(())
}
