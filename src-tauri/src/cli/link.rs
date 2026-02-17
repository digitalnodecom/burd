//! Link CLI commands
//!
//! Commands for linking directories to custom domains from the command line.

use crate::analyzer::{
    analyze_project, detect_project_type, extract_cache_config, extract_database_config,
    extract_mail_config, get_document_root, parse_env_file, update_env_value, ProjectType,
};
use crate::caddy;
use crate::config::{ConfigStore, Domain, Instance, ServiceType};
use crate::db_manager::{create_manager_for_instance, find_all_db_instances, sanitize_db_name};
use chrono::Utc;
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use uuid::Uuid;

/// Link the current directory to a custom domain
///
/// Creates a FrankenPHP instance and domain for the current directory.
/// Also analyzes the project and offers to set up database and fix .env.
pub fn run_link(name: Option<String>) -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Could not determine project name from directory".to_string())?
        .to_string();

    // Detect project type and compute correct document root
    let project_type = detect_project_type(&current_dir);
    let computed_doc_root = get_document_root(&current_dir, &project_type);
    let document_root = computed_doc_root.to_string_lossy().to_string();

    // Inform user if document root differs from project root (e.g., Bedrock /web, Laravel /public)
    if computed_doc_root != current_dir {
        println!(
            "Detected {} project - using document root: {}",
            project_type, document_root
        );
    }

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Use provided name or directory name for subdomain
    // Strip TLD suffix if present (e.g., "hello.burd" -> "hello")
    let subdomain = match name {
        Some(n) => {
            let tld_suffix = format!(".{}", config.tld);
            let stripped = n.strip_suffix(&tld_suffix).unwrap_or(&n);
            slug::slugify(stripped)
        }
        None => slug::slugify(&project_name),
    };

    println!("Linking directory: {}", document_root);

    let mut config = config_store.load()?;
    let project_root = current_dir.to_string_lossy().to_string();

    // Check if already linked (instance with same document_root or project root exists)
    let existing_instance = config.instances.iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == document_root || dr == project_root)
            .unwrap_or(false)
    });

    if let Some(inst) = existing_instance {
        let inst_doc_root = inst
            .config
            .get("document_root")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Check if instance points to wrong document root (project root instead of /web or /public)
        if inst_doc_root == project_root && project_root != document_root {
            return Err(format!(
                "Directory is already linked but points to '{}' instead of '{}'.\n\
                 Run 'burd unlink' first, then 'burd link' to fix the document root.",
                inst_doc_root, document_root
            ));
        }

        return Err(format!(
            "Directory '{}' is already linked.\nUse 'burd unlink' to remove the existing link first.",
            document_root
        ));
    }

    // Check if subdomain already exists
    if config.domains.iter().any(|d| d.subdomain == subdomain) {
        return Err(format!(
            "Domain '{}.{}' already exists.\nUse a different name with: burd link --name <subdomain>",
            subdomain, config.tld
        ));
    }

    // Find an available port (start from default FrankenPHP port)
    let mut port = ServiceType::FrankenPHP.default_port();
    while config.instances.iter().any(|i| i.port == port) {
        if port == u16::MAX {
            return Err("No available ports found".to_string());
        }
        port += 1;
    }

    // Get installed FrankenPHP version
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

    // Create FrankenPHP instance
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

    // Create domain for the instance (SSL disabled by default)
    let domain = Domain::for_instance(subdomain.clone(), instance.id, false);
    config.domains.push(domain);

    // Save config
    config_store.save(&config)?;

    // Build URL
    let url = if config.proxy_installed {
        format!("https://{}.{}", subdomain, config.tld)
    } else {
        format!("http://{}.{}:{}", subdomain, config.tld, config.proxy_port)
    };

    println!();
    println!(
        "Linked '{}' to '{}.{}'",
        project_name, subdomain, config.tld
    );
    println!();
    println!("  URL: {}", url);
    println!("  Port: {}", port);

    // === Project Analysis & Setup ===
    // Reload config to get the latest state
    let config = config_store.load()?;

    if let Ok(project) = analyze_project(&current_dir) {
        if !matches!(project.project_type, ProjectType::Unknown) {
            println!();
            println!("Detected: {}", project.project_type);

            // Offer database setup
            offer_database_setup(&current_dir, &project, &config)?;

            // Offer .env fixes (pass subdomain for site URL check - APP_URL or WP_HOME)
            offer_env_fixes(&current_dir, &project, &config, &subdomain)?;
        }
    }

    println!();
    println!("Start the server with:");
    println!("  Open Burd app and click Start on '{}'", project_name);
    println!();
    println!("Note: Use 'burd unlink' to remove this link.");

    Ok(())
}

/// Offer to set up database for the project
fn offer_database_setup(
    _project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
) -> Result<(), String> {
    // Only offer for projects that use databases
    if !project.project_type.uses_env_file() && !project.project_type.uses_wp_config() {
        return Ok(());
    }

    // Check if project uses SQLite (doesn't need server-based DB)
    if let Some(ref db) = project.database {
        if db.is_sqlite() {
            return Ok(()); // SQLite doesn't need database creation
        }
    }

    // Find database instances
    let db_instances = find_all_db_instances(config);
    if db_instances.is_empty() {
        return Ok(()); // No database service to use
    }

    // Get database name from project config or use project name
    let db_name = project
        .database
        .as_ref()
        .filter(|d| !d.database.is_empty() && !d.is_sqlite())
        .map(|d| d.database.clone())
        .unwrap_or_else(|| {
            sanitize_db_name(&project.name).unwrap_or_else(|_| project.name.clone())
        });

    // Use first database instance (typically MariaDB)
    let db_instance = db_instances[0];
    let manager = create_manager_for_instance(db_instance)?;

    // Check if database already exists
    let db_exists = manager.database_exists(&db_name).unwrap_or(false);

    if db_exists {
        println!();
        println!(
            "Database '{}' already exists on {:?}.",
            db_name, db_instance.service_type
        );
    } else {
        println!();
        print!(
            "Create database '{}' on {:?} (port {})? [Y/n] ",
            db_name, db_instance.service_type, db_instance.port
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {}", e))?;

        if !input.trim().eq_ignore_ascii_case("n") {
            manager.create_database(&db_name)?;
            println!("  Created database '{}'", db_name);
        }
    }

    Ok(())
}

/// Offer to fix .env configuration issues
fn offer_env_fixes(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
    subdomain: &str,
) -> Result<(), String> {
    if !project.project_type.uses_env_file() {
        return Ok(());
    }

    let env_path = project_dir.join(".env");
    if !env_path.exists() {
        // Check for .env.example
        let example_path = project_dir.join(".env.example");
        if example_path.exists() {
            println!();
            print!("No .env file found. Copy from .env.example? [Y/n] ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| format!("Failed to read input: {}", e))?;

            if !input.trim().eq_ignore_ascii_case("n") {
                std::fs::copy(&example_path, &env_path)
                    .map_err(|e| format!("Failed to copy .env.example: {}", e))?;
                println!("  Created .env from .env.example");
            } else {
                return Ok(());
            }
        } else {
            return Ok(());
        }
    }

    let env_vars = match parse_env_file(&env_path) {
        Some(vars) => vars,
        None => return Ok(()),
    };

    let issues = collect_env_issues(&project.project_type, &env_vars, config, subdomain);

    if issues.is_empty() {
        return Ok(());
    }

    println!();
    println!("Found {} .env configuration issue(s):", issues.len());

    for (key, current, suggested, reason) in &issues {
        println!();
        println!("  {} = {} -> {}", key, current, suggested);
        println!("    {}", reason);

        print!("  Apply fix? [y/N] ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {}", e))?;

        if input.trim().eq_ignore_ascii_case("y") {
            update_env_value(&env_path, key, suggested)?;
            println!("    Updated {}", key);
        }
    }

    Ok(())
}

/// Collect .env issues that need fixing
fn collect_env_issues(
    project_type: &ProjectType,
    env_vars: &HashMap<String, String>,
    config: &crate::config::Config,
    subdomain: &str,
) -> Vec<(String, String, String, String)> {
    let mut issues = Vec::new();

    // Check site URL configuration
    // Bedrock uses WP_HOME, Laravel/Symfony use APP_URL
    let url_var_name = if matches!(project_type, ProjectType::Bedrock) {
        "WP_HOME"
    } else {
        "APP_URL"
    };

    let expected_url = if config.proxy_installed {
        format!("http://{}.{}", subdomain, config.tld)
    } else {
        format!("http://{}.{}:{}", subdomain, config.tld, config.proxy_port)
    };

    if let Some(site_url) = env_vars.get(url_var_name) {
        let site_url_normalized = site_url.trim_end_matches('/');
        let expected_normalized = expected_url.trim_end_matches('/');

        if !site_url_normalized.eq_ignore_ascii_case(expected_normalized) {
            issues.push((
                url_var_name.to_string(),
                site_url.clone(),
                expected_url.clone(),
                format!("Should match your Burd domain {}.{}", subdomain, config.tld),
            ));
        }
    } else {
        // Site URL is not set at all
        issues.push((
            url_var_name.to_string(),
            "(not set)".to_string(),
            expected_url.clone(),
            format!("Set to your Burd domain {}.{}", subdomain, config.tld),
        ));
    }

    // Check database configuration
    if let Some(db_config) = extract_database_config(project_type, env_vars) {
        // Find matching Burd database instance
        let db_instance = config.instances.iter().find(|i| {
            if db_config.is_mysql() {
                i.service_type == ServiceType::MariaDB
            } else if db_config.is_postgres() {
                i.service_type == ServiceType::PostgreSQL
            } else {
                false
            }
        });

        if let Some(instance) = db_instance {
            // Check port mismatch
            if db_config.port != instance.port {
                issues.push((
                    "DB_PORT".to_string(),
                    db_config.port.to_string(),
                    instance.port.to_string(),
                    format!(
                        "Burd's {:?} is on port {}",
                        instance.service_type, instance.port
                    ),
                ));
            }

            // Check host
            if db_config.host != "127.0.0.1" && db_config.host != "localhost" {
                issues.push((
                    "DB_HOST".to_string(),
                    db_config.host.clone(),
                    "127.0.0.1".to_string(),
                    "Burd services run locally".to_string(),
                ));
            }
        }
    }

    // Check Redis configuration (Laravel)
    if matches!(project_type, ProjectType::Laravel { .. }) {
        if let Some(cache_config) = extract_cache_config(project_type, env_vars) {
            if cache_config.is_redis() {
                let redis_instance = config
                    .instances
                    .iter()
                    .find(|i| i.service_type == ServiceType::Redis);

                if let Some(instance) = redis_instance {
                    if let Some(port) = cache_config.port {
                        if port != instance.port {
                            issues.push((
                                "REDIS_PORT".to_string(),
                                port.to_string(),
                                instance.port.to_string(),
                                format!("Burd's Redis is on port {}", instance.port),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Check Mail configuration (Laravel)
    if matches!(project_type, ProjectType::Laravel { .. }) {
        if let Some(mail_config) = extract_mail_config(project_type, env_vars) {
            let mailpit_instance = config
                .instances
                .iter()
                .find(|i| i.service_type == ServiceType::Mailpit);

            if let Some(instance) = mailpit_instance {
                let smtp_port = instance
                    .config
                    .get("smtp_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1025) as u16;

                if mail_config.mailer == "smtp" && mail_config.port != smtp_port {
                    issues.push((
                        "MAIL_PORT".to_string(),
                        mail_config.port.to_string(),
                        smtp_port.to_string(),
                        format!("Burd's Mailpit SMTP is on port {}", smtp_port),
                    ));
                }

                if mail_config.mailer == "smtp"
                    && mail_config.host != "127.0.0.1"
                    && mail_config.host != "localhost"
                {
                    issues.push((
                        "MAIL_HOST".to_string(),
                        mail_config.host.clone(),
                        "127.0.0.1".to_string(),
                        "Burd's Mailpit runs locally".to_string(),
                    ));
                }
            }
        }
    }

    issues
}

/// Unlink the current directory
///
/// Removes the domain and instance created by 'burd link' or 'burd init'.
pub fn run_unlink() -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let document_root = current_dir.to_string_lossy().to_string();

    let config_store = ConfigStore::new()?;
    let mut config = config_store.load()?;

    // Find instance with matching document_root OR where current dir is parent
    let instance_idx = config.instances.iter().position(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| {
                // Exact match
                dr == document_root ||
                // Current dir is parent of document_root (e.g., /project vs /project/public)
                std::path::Path::new(dr).starts_with(&current_dir)
            })
            .unwrap_or(false)
    });

    let instance = match instance_idx {
        Some(idx) => config.instances.remove(idx),
        None => {
            // Check if this directory is inside a parked directory
            for parked_dir in &config.parked_directories {
                if document_root.starts_with(&parked_dir.path) && document_root != parked_dir.path {
                    return Err(format!(
                        "This directory is inside a parked directory: {}\n\
                         Use 'burd forget' in the parent directory to unpark, or remove the project folder.",
                        parked_dir.path
                    ));
                }
            }
            return Err(format!(
                "Directory '{}' is not linked.\nUse 'burd link' to link it first.",
                document_root
            ));
        }
    };

    // Find and remove domain pointing to this instance
    let domain_idx = config
        .domains
        .iter()
        .position(|d| d.routes_to_instance(&instance.id));

    if let Some(idx) = domain_idx {
        let domain = config.domains.remove(idx);
        let full_domain = format!("{}.{}", domain.subdomain, config.tld);

        // Delete Caddy domain file
        if let Err(e) = caddy::delete_domain_file(&full_domain) {
            eprintln!(
                "Warning: Failed to delete domain file for {}: {}",
                full_domain, e
            );
        }
    }

    // Delete instance directory
    if let Ok(instance_dir) = crate::config::get_instance_dir(&instance.id) {
        if instance_dir.exists() {
            let _ = std::fs::remove_dir_all(&instance_dir);
        }
    }

    // Save config
    config_store.save(&config)?;

    println!();
    println!("Unlinked '{}'", instance.name);
    println!();

    Ok(())
}

/// List all linked sites
///
/// Shows all directories linked via 'burd link' or 'burd init'.
pub fn run_links() -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Find all FrankenPHP instances with document_root set (these are linked sites)
    let linked_instances: Vec<&Instance> = config
        .instances
        .iter()
        .filter(|i| {
            i.service_type == ServiceType::FrankenPHP
                && i.config
                    .get("document_root")
                    .and_then(|v| v.as_str())
                    .is_some()
        })
        .collect();

    if linked_instances.is_empty() {
        println!("No linked sites found.");
        println!();
        println!("Link a directory with: burd link");
        return Ok(());
    }

    println!("Linked Sites:");
    println!();

    for instance in linked_instances {
        let document_root = instance
            .config
            .get("document_root")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Find associated domain
        let domain = config
            .domains
            .iter()
            .find(|d| d.routes_to_instance(&instance.id));

        let domain_str = domain
            .map(|d| format!("{}.{}", d.subdomain, config.tld))
            .unwrap_or_else(|| "no domain".to_string());

        let ssl_status = domain
            .map(|d| if d.ssl_enabled { "SSL" } else { "HTTP" })
            .unwrap_or("HTTP");

        println!(
            "  {} -> {} (port {}, {})",
            document_root, domain_str, instance.port, ssl_status
        );
    }

    println!();
    Ok(())
}
