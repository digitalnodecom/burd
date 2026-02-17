//! Doctor CLI command
//!
//! Health check for Burd services and current project configuration.
//! Diagnoses issues and provides actionable suggestions.

use crate::analyzer::{
    analyze_project, extract_cache_config, extract_database_config, extract_mail_config,
    parse_env_file, ProjectType,
};
use crate::config::{ConfigStore, ServiceType};
use crate::db_manager::{create_manager_for_instance, find_all_db_instances};
use std::env;
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

/// Health check status
#[derive(Debug, Clone, Copy, PartialEq)]
enum Status {
    Ok,
    Warning,
    Error,
    NotInstalled,
}

impl Status {
    fn symbol(&self) -> &'static str {
        match self {
            Status::Ok => "[OK]",
            Status::Warning => "[WARN]",
            Status::Error => "[ERR]",
            Status::NotInstalled => "[--]",
        }
    }
}

/// Run health check
///
/// Checks:
/// - Burd service instances (running, ports available)
/// - Current project configuration vs Burd services
/// - Database connectivity and existence
/// - Cache configuration
/// - Mail configuration
pub fn run_doctor() -> Result<(), String> {
    println!();
    println!("Burd Health Check");
    println!("=================");

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // === Section 1: Services ===
    println!();
    println!("Services");
    println!("--------");

    let mut has_frankenphp = false;
    let mut has_mariadb = false;
    let mut has_postgres = false;
    let mut has_redis = false;
    let mut has_mailpit = false;
    let mut has_meilisearch = false;

    for instance in &config.instances {
        let port_open = check_port(instance.port);
        let status = if port_open { Status::Ok } else { Status::Error };
        let status_text = if port_open {
            "running"
        } else {
            "not responding"
        };

        println!(
            "  {} {:?} '{}' (port {}) - {}",
            status.symbol(),
            instance.service_type,
            instance.name,
            instance.port,
            status_text
        );

        match instance.service_type {
            ServiceType::FrankenPHP | ServiceType::FrankenPhpPark => has_frankenphp = true,
            ServiceType::MariaDB => has_mariadb = true,
            ServiceType::PostgreSQL => has_postgres = true,
            ServiceType::Redis => has_redis = true,
            ServiceType::Mailpit => has_mailpit = true,
            ServiceType::Meilisearch => has_meilisearch = true,
            _ => {}
        }
    }

    if config.instances.is_empty() {
        println!("  No services configured.");
        println!();
        println!("  Add services in the Burd app to get started.");
    }

    // Show missing common services
    println!();
    println!("Service Coverage");
    println!("----------------");

    print_service_status("PHP Server", has_frankenphp);
    print_service_status("Database (MariaDB)", has_mariadb);
    print_service_status("Database (PostgreSQL)", has_postgres);
    print_service_status("Cache (Redis)", has_redis);
    print_service_status("Mail (Mailpit)", has_mailpit);
    print_service_status("Search (Meilisearch)", has_meilisearch);

    // === Section 2: Proxy ===
    println!();
    println!("Proxy");
    println!("-----");

    if config.proxy_installed {
        let proxy_running = check_port(443);
        let status = if proxy_running {
            Status::Ok
        } else {
            Status::Warning
        };
        println!(
            "  {} Caddy proxy installed (HTTPS on port 443)",
            status.symbol()
        );
        if !proxy_running {
            println!("      Proxy may not be running. Check System Preferences > Burd.");
        }
    } else {
        println!(
            "  {} Caddy proxy not installed",
            Status::NotInstalled.symbol()
        );
        println!(
            "      Sites accessible via http://site.{}:{}",
            config.tld, config.proxy_port
        );
        println!("      Install proxy in Burd app for HTTPS support.");
    }

    // === Section 3: Current Project ===
    println!();
    println!("Current Project");
    println!("---------------");

    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    match analyze_project(&current_dir) {
        Ok(project) => {
            if matches!(project.project_type, ProjectType::Unknown) {
                println!(
                    "  {} No recognized project in current directory",
                    Status::NotInstalled.symbol()
                );
                println!("      Supported: Laravel, WordPress, Bedrock, Symfony");
            } else {
                println!("  Type: {}", project.project_type);
                println!("  Path: {}", project.path.display());
                println!("  Document Root: {}", project.document_root.display());

                // Check if linked
                let is_linked = config.instances.iter().any(|i| {
                    i.config
                        .get("document_root")
                        .and_then(|v| v.as_str())
                        .map(|dr| dr == current_dir.to_string_lossy())
                        .unwrap_or(false)
                });

                if is_linked {
                    let subdomain = slug::slugify(&project.name);
                    println!(
                        "  {} Linked to {}.{}",
                        Status::Ok.symbol(),
                        subdomain,
                        config.tld
                    );
                } else {
                    println!("  {} Not linked", Status::Warning.symbol());
                    println!("      Run 'burd link' or 'burd setup' to link this project.");
                }

                // Check project-specific things
                println!();
                check_project_database(&current_dir, &project, &config)?;
                check_project_cache(&current_dir, &project, &config)?;
                check_project_mail(&current_dir, &project, &config)?;
            }
        }
        Err(_) => {
            println!(
                "  {} No project detected in current directory",
                Status::NotInstalled.symbol()
            );
        }
    }

    // === Summary ===
    println!();
    println!("Legend: [OK] = Good, [WARN] = Warning, [ERR] = Error, [--] = Not installed");

    Ok(())
}

/// Print service installation status
fn print_service_status(name: &str, installed: bool) {
    let status = if installed {
        Status::Ok
    } else {
        Status::NotInstalled
    };
    let text = if installed {
        "configured"
    } else {
        "not configured"
    };
    println!("  {} {} - {}", status.symbol(), name, text);
}

/// Check if a port is open (service is listening)
fn check_port(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_millis(500),
    )
    .is_ok()
}

/// Check project database configuration
fn check_project_database(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
) -> Result<(), String> {
    println!("Database:");

    // Get database config from project
    let db_config = if project.project_type.uses_env_file() {
        let env_path = project_dir.join(".env");
        if env_path.exists() {
            parse_env_file(&env_path)
                .and_then(|vars| extract_database_config(&project.project_type, &vars))
        } else {
            None
        }
    } else {
        project.database.clone()
    };

    let db_config = match db_config {
        Some(db) => db,
        None => {
            println!(
                "    {} No database configuration found",
                Status::Warning.symbol()
            );
            return Ok(());
        }
    };

    // SQLite doesn't need server
    if db_config.is_sqlite() {
        println!(
            "    {} Using SQLite (no server needed)",
            Status::Ok.symbol()
        );
        return Ok(());
    }

    // Find matching Burd instance
    let db_instances = find_all_db_instances(config);
    let matching_instance = db_instances.iter().find(|i| i.port == db_config.port);

    match matching_instance {
        Some(instance) => {
            println!(
                "    {} Config points to {:?} on port {}",
                Status::Ok.symbol(),
                instance.service_type,
                instance.port
            );

            // Check if database exists
            if let Ok(manager) = create_manager_for_instance(instance) {
                match manager.database_exists(&db_config.database) {
                    Ok(true) => {
                        println!(
                            "    {} Database '{}' exists",
                            Status::Ok.symbol(),
                            db_config.database
                        );
                    }
                    Ok(false) => {
                        println!(
                            "    {} Database '{}' does not exist",
                            Status::Warning.symbol(),
                            db_config.database
                        );
                        println!("        Run: burd db create {}", db_config.database);
                    }
                    Err(_) => {
                        println!(
                            "    {} Could not check if database '{}' exists",
                            Status::Warning.symbol(),
                            db_config.database
                        );
                        println!("        Is {:?} running?", instance.service_type);
                    }
                }
            }
        }
        None => {
            // Check if there's a Burd database on a different port
            if let Some(burd_db) = db_instances.first() {
                println!(
                    "    {} Config uses port {}, but Burd's {:?} is on port {}",
                    Status::Warning.symbol(),
                    db_config.port,
                    burd_db.service_type,
                    burd_db.port
                );
                println!("        Run: burd env fix");
            } else {
                println!(
                    "    {} Config uses port {}, no Burd database configured",
                    Status::Warning.symbol(),
                    db_config.port
                );
            }
        }
    }

    Ok(())
}

/// Check project cache configuration
fn check_project_cache(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
) -> Result<(), String> {
    // Only check for Laravel
    if !matches!(project.project_type, ProjectType::Laravel { .. }) {
        return Ok(());
    }

    println!();
    println!("Cache:");

    let env_path = project_dir.join(".env");
    if !env_path.exists() {
        println!("    {} No .env file", Status::Warning.symbol());
        return Ok(());
    }

    let env_vars = match parse_env_file(&env_path) {
        Some(vars) => vars,
        None => return Ok(()),
    };

    let cache_config = extract_cache_config(&project.project_type, &env_vars);

    let cache_driver = env_vars
        .get("CACHE_STORE")
        .or(env_vars.get("CACHE_DRIVER"))
        .map(|s| s.as_str())
        .unwrap_or("file");

    if cache_driver == "redis" {
        let redis_instance = config
            .instances
            .iter()
            .find(|i| i.service_type == ServiceType::Redis);

        match redis_instance {
            Some(instance) => {
                if let Some(cache) = cache_config {
                    if cache.port == Some(instance.port) {
                        println!(
                            "    {} Using Redis on port {}",
                            Status::Ok.symbol(),
                            instance.port
                        );
                    } else {
                        println!(
                            "    {} Config uses port {:?}, Burd's Redis is on port {}",
                            Status::Warning.symbol(),
                            cache.port,
                            instance.port
                        );
                        println!("        Run: burd env fix");
                    }
                } else {
                    println!("    {} Using Redis", Status::Ok.symbol());
                }
            }
            None => {
                println!(
                    "    {} CACHE_STORE=redis but no Redis configured in Burd",
                    Status::Warning.symbol()
                );
                println!("        Add Redis in the Burd app, or change CACHE_STORE to 'file'");
            }
        }
    } else {
        println!(
            "    {} Using '{}' cache driver",
            Status::Ok.symbol(),
            cache_driver
        );
    }

    Ok(())
}

/// Check project mail configuration
fn check_project_mail(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
) -> Result<(), String> {
    // Only check for Laravel
    if !matches!(project.project_type, ProjectType::Laravel { .. }) {
        return Ok(());
    }

    println!();
    println!("Mail:");

    let env_path = project_dir.join(".env");
    if !env_path.exists() {
        return Ok(());
    }

    let env_vars = match parse_env_file(&env_path) {
        Some(vars) => vars,
        None => return Ok(()),
    };

    let mail_config = extract_mail_config(&project.project_type, &env_vars);

    let mail_mailer = env_vars
        .get("MAIL_MAILER")
        .map(|s| s.as_str())
        .unwrap_or("log");

    if mail_mailer == "smtp" {
        let mailpit_instance = config
            .instances
            .iter()
            .find(|i| i.service_type == ServiceType::Mailpit);

        match mailpit_instance {
            Some(instance) => {
                let smtp_port = instance
                    .config
                    .get("smtp_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1025) as u16;

                if let Some(mail) = mail_config {
                    if mail.host == "127.0.0.1" && mail.port == smtp_port {
                        let web_port = instance
                            .config
                            .get("web_port")
                            .and_then(|v| v.as_u64())
                            .unwrap_or(8025);
                        println!(
                            "    {} Using Mailpit (SMTP {}, Web http://localhost:{})",
                            Status::Ok.symbol(),
                            smtp_port,
                            web_port
                        );
                    } else {
                        println!(
                            "    {} SMTP configured for {}:{}, Burd's Mailpit is on port {}",
                            Status::Warning.symbol(),
                            mail.host,
                            mail.port,
                            smtp_port
                        );
                        println!("        Run: burd env fix");
                    }
                }
            }
            None => {
                if let Some(mail) = mail_config {
                    println!(
                        "    {} Using SMTP at {}:{}",
                        Status::Ok.symbol(),
                        mail.host,
                        mail.port
                    );
                    println!("        Consider adding Mailpit in Burd for local mail testing.");
                }
            }
        }
    } else if mail_mailer == "log" {
        println!(
            "    {} Using 'log' mailer (emails logged, not sent)",
            Status::Ok.symbol()
        );
    } else {
        println!("    {} Using '{}' mailer", Status::Ok.symbol(), mail_mailer);
    }

    Ok(())
}
