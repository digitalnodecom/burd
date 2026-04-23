//! `burd update [NAME] --php-version VER [--port N] [--name NEW]`
//!
//! Mirrors the MCP `update_instance` tool. Today only `--php-version` is
//! wired because that's the documented CLI gap; the PUT /instances/:id
//! endpoint accepts more fields, which can be added later as flags.

use crate::api_client::BurdApiClient;
use crate::cli::lifecycle::resolve_instance;
use crate::config::{ConfigStore, ServiceType};
use serde_json::{json, Map, Value};

pub struct UpdateOptions {
    pub php_version: Option<String>,
    pub port: Option<u16>,
    pub new_name: Option<String>,
}

pub fn run_update(name: Option<String>, opts: UpdateOptions) -> Result<(), String> {
    if opts.php_version.is_none() && opts.port.is_none() && opts.new_name.is_none() {
        return Err(
            "Nothing to update. Pass at least one of: --php-version, --port, --name.".to_string(),
        );
    }

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let instance = resolve_instance(&config, name.as_deref())?;

    if opts.php_version.is_some() && instance.service_type != ServiceType::FrankenPHP {
        return Err(format!(
            "--php-version only applies to FrankenPHP instances. '{}' is {:?}.",
            instance.name, instance.service_type
        ));
    }

    let client = BurdApiClient::new();
    if !client.is_available() {
        return Err(
            "Burd app isn't running. Open Burd or run `burd setup`, then try again.".to_string(),
        );
    }

    let mut body = Map::new();
    if let Some(v) = opts.php_version.as_ref() {
        body.insert("version".to_string(), json!(v));
    }
    if let Some(p) = opts.port {
        body.insert("port".to_string(), json!(p));
    }
    if let Some(n) = opts.new_name.as_ref() {
        body.insert("name".to_string(), json!(n));
    }

    client.put(&format!("/instances/{}", instance.id), &Value::Object(body))?;

    println!("✓ Updated '{}'", instance.name);
    if let Some(v) = opts.php_version {
        println!("  php version → {}", v);
    }
    if let Some(p) = opts.port {
        println!("  port → {}", p);
    }
    if let Some(n) = opts.new_name {
        println!("  name → {}", n);
    }
    Ok(())
}
