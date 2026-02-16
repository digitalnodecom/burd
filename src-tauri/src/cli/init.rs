//! The `burd init` command implementation
//!
//! Creates a FrankenPHP instance + domain for the current directory.

use crate::config::{ConfigStore, Domain, Instance, ServiceType};
use chrono::Utc;
use std::env;
use std::path::PathBuf;
use uuid::Uuid;

/// Run the init command
///
/// Creates a FrankenPHP instance with document_root set to the current directory
/// and a domain based on the directory name.
pub fn run_init() -> Result<(), String> {
    // 1. Get current directory
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Could not determine project name from directory".to_string())?
        .to_string();

    let document_root = current_dir.to_string_lossy().to_string();

    println!("Initializing Burd in: {}", document_root);

    // 2. Load config
    let config_store = ConfigStore::new()?;
    let mut config = config_store.load()?;

    // 3. Generate subdomain from project name
    let subdomain = slug::slugify(&project_name);

    // Check if subdomain already exists
    if config.domains.iter().any(|d| d.subdomain == subdomain) {
        return Err(format!(
            "Domain '{}.{}' already exists. Use a different directory or remove the existing domain.",
            subdomain, config.tld
        ));
    }

    // 4. Find an available port (start from default FrankenPHP port)
    let mut port = ServiceType::FrankenPHP.default_port();
    while config.instances.iter().any(|i| i.port == port) {
        if port == u16::MAX {
            return Err("No available ports found".to_string());
        }
        port += 1;
    }

    // 5. Get installed FrankenPHP version
    let version = config.binaries
        .get(&ServiceType::FrankenPHP)
        .and_then(|versions| versions.keys().next())
        .ok_or_else(|| {
            "No FrankenPHP versions installed.\n\
             Please download FrankenPHP in the Burd app first.".to_string()
        })?
        .clone();

    // 6. Create FrankenPHP instance
    let instance = Instance {
        id: Uuid::new_v4(),
        name: project_name.clone(),
        port,
        service_type: ServiceType::FrankenPHP,
        version,
        config: serde_json::json!({
            "document_root": document_root
        }),
        master_key: None,
        auto_start: false,
        created_at: Utc::now(),
        domain: Some(subdomain.clone()),
        domain_enabled: true,
        stack_id: None,
    };

    // Create instance data directory
    let instance_dir = crate::config::get_instance_dir(&instance.id)?;
    std::fs::create_dir_all(&instance_dir)
        .map_err(|e| format!("Failed to create instance directory: {}", e))?;

    config.instances.push(instance.clone());

    // 7. Create domain for the instance (SSL disabled by default)
    let domain = Domain::for_instance(subdomain.clone(), instance.id, false);
    config.domains.push(domain);

    // 8. Save config
    config_store.save(&config)?;

    // 9. Print success
    let url = if config.proxy_installed {
        format!("https://{}.{}", subdomain, config.tld)
    } else {
        format!("http://{}.{}:{}", subdomain, config.tld, config.proxy_port)
    };

    println!();
    println!("✓ Created instance '{}' (FrankenPHP on port {})", project_name, port);
    println!("✓ Created domain '{}.{}'", subdomain, config.tld);
    println!();
    println!("  URL: {}", url);
    println!();
    println!("Start the server with:");
    println!("  Open Burd app and click Start on '{}'", project_name);
    println!();
    println!("Note: Make sure FrankenPHP is downloaded in the Burd app first.");

    Ok(())
}

/// Check if burd is already initialized in the current directory
pub fn is_initialized() -> Result<Option<Instance>, String> {
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let document_root = current_dir.to_string_lossy().to_string();

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Find an instance with matching document_root
    let instance = config.instances.into_iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == document_root)
            .unwrap_or(false)
    });

    Ok(instance)
}

/// Get the config file path for display
pub fn get_config_path() -> Result<PathBuf, String> {
    crate::config::get_app_dir().map(|p| p.join("config.json"))
}
