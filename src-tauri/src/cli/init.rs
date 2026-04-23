//! The `burd init` command implementation
//!
//! Creates a FrankenPHP instance + domain for the current directory, with
//! framework-aware document root (Laravel/Symfony → `public/`, Bedrock → `web/`),
//! SSL enabled by default, and auto-start by default.

use crate::analyzer::{detect_project_type, get_document_root};
use crate::api_client::BurdApiClient;
use crate::caddy;
use crate::config::{ConfigStore, Domain, Instance, ServiceType};
use chrono::Utc;
use std::env;
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// Options for `burd init`, mirrored from the CLI flags.
#[derive(Debug, Default)]
pub struct InitOptions {
    pub no_ssl: bool,
    pub no_start: bool,
    pub public_dir: Option<PathBuf>,
}

/// Run the init command with default options.
pub fn run_init() -> Result<(), String> {
    run_init_with(InitOptions::default())
}

/// Run the init command.
pub fn run_init_with(opts: InitOptions) -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    // Refuse re-init when this directory is already a Burd project — re-running
    // init would either duplicate state or silently overwrite. Point the user
    // at `burd start` so they don't think they need to re-run init to recover.
    if let Some(existing) = is_initialized()? {
        return Err(format!(
            "This directory is already initialized as instance '{}'.\n\
             Run `burd start` to start it, or `burd unlink` to remove it first.",
            existing.name
        ));
    }

    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Could not determine project name from directory".to_string())?
        .to_string();

    // Resolve doc root: explicit override wins, else framework detection.
    let project_type = detect_project_type(&current_dir);
    let doc_root_path: PathBuf = match opts.public_dir.as_ref() {
        Some(p) => resolve_override_dir(&current_dir, p)?,
        None => get_document_root(&current_dir, &project_type),
    };
    let document_root = doc_root_path.to_string_lossy().to_string();

    if doc_root_path != current_dir {
        println!(
            "Detected {} project — document root: {}",
            project_type, document_root
        );
    } else {
        println!("Initializing Burd in: {}", document_root);
    }

    let config_store = ConfigStore::new()?;
    let mut config = config_store.load()?;

    let subdomain = slug::slugify(&project_name);

    if config.domains.iter().any(|d| d.subdomain == subdomain) {
        return Err(format!(
            "Domain '{}.{}' already exists. Use `burd start` to run it, or remove it first.",
            subdomain, config.tld
        ));
    }

    let mut port = ServiceType::FrankenPHP.default_port();
    while config.instances.iter().any(|i| i.port == port) {
        if port == u16::MAX {
            return Err("No available ports found".to_string());
        }
        port += 1;
    }

    let version = config
        .binaries
        .get(&ServiceType::FrankenPHP)
        .and_then(|versions| versions.keys().next())
        .ok_or_else(|| {
            "No FrankenPHP versions installed.\n\
             Please download FrankenPHP in the Burd app first."
                .to_string()
        })?
        .clone();

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

    let instance_dir = crate::config::get_instance_dir(&instance.id)?;
    std::fs::create_dir_all(&instance_dir)
        .map_err(|e| format!("Failed to create instance directory: {}", e))?;

    config.instances.push(instance.clone());

    let ssl_enabled = !opts.no_ssl;
    let domain = Domain::for_instance(subdomain.clone(), instance.id, ssl_enabled);
    let domain_id = domain.id;
    config.domains.push(domain);

    config_store.save(&config)?;

    // Previously `burd init` wrote the domain to config but never generated
    // the Caddy domain file, so Caddy had no route until the daemon next
    // regenerated from scratch. Write it here so the site is reachable
    // immediately after auto-start.
    let full_domain = format!("{}.{}", subdomain, config.tld);
    let route = caddy::RouteEntry::reverse_proxy(
        full_domain.clone(),
        port,
        domain_id.to_string(),
        ssl_enabled,
    );
    if let Err(e) = caddy::write_domain_file(&route) {
        eprintln!("Warning: failed to write Caddy domain file for {}: {}", full_domain, e);
    }

    let scheme = if ssl_enabled { "https" } else { "http" };
    let url = if config.proxy_installed {
        format!("{}://{}.{}", scheme, subdomain, config.tld)
    } else {
        format!(
            "{}://{}.{}:{}",
            scheme, subdomain, config.tld, config.proxy_port
        )
    };

    println!();
    println!(
        "✓ Created instance '{}' (FrankenPHP on port {})",
        project_name, port
    );
    println!(
        "✓ Created domain '{}.{}' ({})",
        subdomain,
        config.tld,
        if ssl_enabled { "SSL" } else { "HTTP" }
    );

    // Seed `.env` from `.env.example` so first-run `php artisan migrate` etc.
    // don't die on a missing file. Skipped when `.env` already exists — we
    // never overwrite user config.
    seed_env_from_example(&current_dir);

    if opts.no_start {
        println!();
        println!("  URL: {}", url);
        println!();
        println!("Start the server with:  burd start");
        return Ok(());
    }

    // Auto-start via daemon API.
    let client = BurdApiClient::new();
    if !client.is_available() {
        println!();
        println!("  URL: {}", url);
        println!();
        println!(
            "The Burd app is not running — the site is configured but not started.\n\
             Open Burd or run `burd start` once it's running."
        );
        return Ok(());
    }

    let path = format!("/instances/{}/start", instance.id);
    match client.post(&path, &serde_json::json!({})) {
        Ok(_) => {
            println!("✓ Started");
            println!();
            println!("  URL: {}", url);
        }
        Err(e) => {
            println!();
            println!("  URL: {}", url);
            return Err(format!("Instance created but failed to start: {}", e));
        }
    }

    Ok(())
}

/// Copy `.env.example` to `.env` on a fresh checkout. Intentionally silent on
/// success (init output is already noisy) and non-fatal on failure — we don't
/// want a locked filesystem or unreadable example file to block instance setup.
fn seed_env_from_example(dir: &Path) {
    let env_path = dir.join(".env");
    let example_path = dir.join(".env.example");
    if env_path.exists() || !example_path.exists() {
        return;
    }
    match std::fs::copy(&example_path, &env_path) {
        Ok(_) => println!("✓ Seeded .env from .env.example"),
        Err(e) => eprintln!("Warning: failed to seed .env: {}", e),
    }
}

fn resolve_override_dir(base: &Path, override_path: &Path) -> Result<PathBuf, String> {
    let candidate = if override_path.is_absolute() {
        override_path.to_path_buf()
    } else {
        base.join(override_path)
    };
    if !candidate.is_dir() {
        return Err(format!(
            "--public-dir: {} is not a directory",
            candidate.display()
        ));
    }
    Ok(candidate)
}

/// Check if burd is already initialized in the current directory.
pub fn is_initialized() -> Result<Option<Instance>, String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let document_root = current_dir.to_string_lossy().to_string();

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let instance = config.instances.into_iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| {
                dr == document_root
                    || Path::new(dr)
                        .parent()
                        .map(|p| p == current_dir)
                        .unwrap_or(false)
            })
            .unwrap_or(false)
    });

    Ok(instance)
}

/// Get the config file path for display.
pub fn get_config_path() -> Result<PathBuf, String> {
    crate::config::get_app_dir().map(|p| p.join("config.json"))
}
