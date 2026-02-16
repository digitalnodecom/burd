//! Project Analyzer Module
//!
//! Analyzes PHP projects to detect their type, configuration, and suggest
//! optimizations for use with Burd's services.

pub mod detector;
pub mod parsers;
pub mod types;

pub use detector::{detect_project_type, get_document_root};
pub use parsers::{
    extract_cache_config, extract_database_config, extract_mail_config, extract_php_version,
    extract_search_config, parse_composer_json, parse_env_file, parse_wp_config, update_env_value,
};
pub use types::{
    CacheConfig, ComposerInfo, DatabaseConfig, IssueSeverity, MailConfig, ProjectInfo,
    ProjectIssue, ProjectType, SearchConfig,
};

use crate::config::{Config, Instance, ServiceType};
use std::path::Path;

/// Analyze a project directory
///
/// Detects project type, parses configuration files, and identifies
/// potential issues or improvements.
pub fn analyze_project(path: &Path) -> Result<ProjectInfo, String> {
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    if !path.is_dir() {
        return Err(format!("Path is not a directory: {}", path.display()));
    }

    // Detect project type
    let project_type = detect_project_type(path);
    let document_root = get_document_root(path, &project_type);

    // Get project name from directory
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    // Start building project info
    let mut info = ProjectInfo {
        project_type: project_type.clone(),
        name,
        path: path.to_path_buf(),
        document_root,
        php_version: None,
        instance_php_version: None,
        database: None,
        cache: None,
        mail: None,
        search: None,
        env_file: None,
        issues: Vec::new(),
    };

    // Parse composer.json for PHP version
    if let Some(composer) = parse_composer_json(path) {
        info.php_version = extract_php_version(&composer);
    }

    // Parse configuration based on project type
    match &project_type {
        ProjectType::Laravel { .. } | ProjectType::Bedrock | ProjectType::Symfony { .. } => {
            // These use .env files
            let env_path = path.join(".env");
            if env_path.exists() {
                info.env_file = Some(env_path.clone());
                if let Some(env) = parse_env_file(&env_path) {
                    info.database = extract_database_config(&project_type, &env);
                    info.cache = extract_cache_config(&project_type, &env);
                    info.mail = extract_mail_config(&project_type, &env);
                    info.search = extract_search_config(&project_type, &env);
                }
            } else {
                // Check for .env.example
                let example_path = path.join(".env.example");
                if example_path.exists() {
                    info.add_issue(
                        ProjectIssue::warning("config", ".env file not found")
                            .with_suggestion("Copy .env.example to .env and configure it"),
                    );
                }
            }
        }
        ProjectType::WordPress => {
            // Parse wp-config.php
            info.database = parse_wp_config(path);

            if info.database.is_none() {
                let sample_path = path.join("wp-config-sample.php");
                if sample_path.exists() {
                    info.add_issue(
                        ProjectIssue::warning("config", "wp-config.php not found")
                            .with_suggestion("Copy wp-config-sample.php to wp-config.php and configure database settings"),
                    );
                }
            }
        }
        ProjectType::Unknown => {
            info.add_issue(ProjectIssue::info(
                "project",
                "Could not detect project type",
            ));
        }
    }

    Ok(info)
}

/// Analyze project and check against Burd configuration
///
/// Compares project settings with running Burd services and
/// generates suggestions for alignment.
pub fn analyze_with_burd_config(path: &Path, config: &Config) -> Result<ProjectInfo, String> {
    let mut info = analyze_project(path)?;

    // Find FrankenPHP instance linked to this project for the actual running PHP version
    let project_root = path.to_string_lossy().to_string();
    let computed_doc_root = info.document_root.to_string_lossy().to_string();

    let php_instance = config.instances.iter().find(|i| {
        matches!(
            i.service_type,
            ServiceType::FrankenPHP | ServiceType::FrankenPhpPark
        ) && i
            .config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == project_root || dr == computed_doc_root)
            .unwrap_or(false)
    });

    if let Some(inst) = php_instance {
        if !inst.version.is_empty() {
            info.instance_php_version = Some(inst.version.clone());
        }
    }

    // Clone configs to avoid borrow checker issues
    let db = info.database.clone();
    let cache = info.cache.clone();
    let mail = info.mail.clone();
    let search = info.search.clone();

    // Check database configuration
    if let Some(ref db) = db {
        check_database_config(&mut info, db, config);
    }

    // Check cache configuration
    if let Some(ref cache) = cache {
        check_cache_config(&mut info, cache, config);
    }

    // Check mail configuration
    if let Some(ref mail) = mail {
        check_mail_config(&mut info, mail, config);
    }

    // Check search configuration
    if let Some(ref search) = search {
        check_search_config(&mut info, search, config);
    }

    // Check APP_URL configuration
    check_app_url_config(&mut info, path, config);

    // Check document root configuration
    check_document_root_config(&mut info, path, config);

    Ok(info)
}

/// Check database configuration against Burd instances
fn check_database_config(info: &mut ProjectInfo, db: &DatabaseConfig, config: &Config) {
    // Find Burd database instances
    let db_instances: Vec<&Instance> = config
        .instances
        .iter()
        .filter(|i| {
            matches!(
                i.service_type,
                ServiceType::MariaDB | ServiceType::PostgreSQL
            )
        })
        .collect();

    if db_instances.is_empty() {
        info.add_issue(
            ProjectIssue::info("database", "No database service configured in Burd")
                .with_suggestion("Create a MariaDB or PostgreSQL instance in the Burd app"),
        );
        return;
    }

    // Check if project's DB port matches any Burd instance
    let matching_instance = db_instances.iter().find(|i| i.port == db.port);

    if matching_instance.is_none() {
        // Find the right type of database
        let suggested_instance = if db.is_mysql() {
            db_instances
                .iter()
                .find(|i| i.service_type == ServiceType::MariaDB)
        } else if db.is_postgres() {
            db_instances
                .iter()
                .find(|i| i.service_type == ServiceType::PostgreSQL)
        } else {
            None
        };

        if let Some(inst) = suggested_instance {
            info.add_issue(
                ProjectIssue::warning(
                    "database",
                    format!(
                        "Database port {} doesn't match Burd's {:?} on port {}",
                        db.port, inst.service_type, inst.port
                    ),
                )
                .with_suggestion(format!("Update DB_PORT to {} in .env", inst.port)),
            );
        }
    }
}

/// Check cache configuration against Burd instances
fn check_cache_config(info: &mut ProjectInfo, cache: &CacheConfig, config: &Config) {
    if !cache.is_redis() {
        return; // Only check Redis for now
    }

    // Find Burd Redis instance
    let redis_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Redis);

    match redis_instance {
        Some(inst) => {
            if let Some(port) = cache.port {
                if port != inst.port {
                    info.add_issue(
                        ProjectIssue::warning(
                            "cache",
                            format!(
                                "Redis port {} doesn't match Burd's Redis on port {}",
                                port, inst.port
                            ),
                        )
                        .with_suggestion(format!("Update REDIS_PORT to {} in .env", inst.port)),
                    );
                }
            }
        }
        None => {
            info.add_issue(
                ProjectIssue::info("cache", "Project uses Redis but no Redis instance in Burd")
                    .with_suggestion("Create a Redis instance in the Burd app"),
            );
        }
    }
}

/// Check mail configuration against Burd instances
fn check_mail_config(info: &mut ProjectInfo, mail: &MailConfig, config: &Config) {
    if mail.mailer != "smtp" {
        return; // Only check SMTP config
    }

    // Find Burd Mailpit instance
    let mailpit_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Mailpit);

    match mailpit_instance {
        Some(inst) => {
            // Mailpit SMTP is typically on port 1025, web UI on inst.port
            let smtp_port = inst
                .config
                .get("smtp_port")
                .and_then(|v| v.as_u64())
                .unwrap_or(1025) as u16;

            if mail.port != smtp_port {
                info.add_issue(
                    ProjectIssue::info(
                        "mail",
                        format!(
                            "Mail port {} doesn't match Burd's Mailpit SMTP on port {}",
                            mail.port, smtp_port
                        ),
                    )
                    .with_suggestion(format!(
                        "Update MAIL_HOST=127.0.0.1 and MAIL_PORT={} in .env",
                        smtp_port
                    )),
                );
            }
        }
        None => {
            if mail.is_mailtrap() {
                info.add_issue(
                    ProjectIssue::info(
                        "mail",
                        "Project uses Mailtrap but Burd has Mailpit for local mail testing",
                    )
                    .with_suggestion("Consider creating a Mailpit instance in Burd for local mail"),
                );
            }
        }
    }
}

/// Check search configuration against Burd instances
fn check_search_config(info: &mut ProjectInfo, search: &SearchConfig, config: &Config) {
    if !search.is_meilisearch() {
        return;
    }

    // Find Burd Meilisearch instance
    let meili_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Meilisearch);

    if meili_instance.is_none() {
        info.add_issue(
            ProjectIssue::warning(
                "search",
                "Project uses Meilisearch but no Meilisearch instance in Burd",
            )
            .with_suggestion("Create a Meilisearch instance in the Burd app"),
        );
    }
}

/// Check site URL configuration against Burd domains
/// Uses APP_URL for Laravel/Symfony, WP_HOME for Bedrock
fn check_app_url_config(info: &mut ProjectInfo, path: &Path, config: &Config) {
    // Only check for projects that use .env files
    if !matches!(
        info.project_type,
        ProjectType::Laravel { .. } | ProjectType::Bedrock | ProjectType::Symfony { .. }
    ) {
        return;
    }

    // Get the appropriate URL variable from .env
    let env_path = path.join(".env");
    let env_vars = match parse_env_file(&env_path) {
        Some(vars) => vars,
        None => return,
    };

    // Bedrock uses WP_HOME, Laravel/Symfony use APP_URL
    let (url_var_name, site_url) = if matches!(info.project_type, ProjectType::Bedrock) {
        ("WP_HOME", env_vars.get("WP_HOME"))
    } else {
        ("APP_URL", env_vars.get("APP_URL"))
    };

    let site_url = match site_url {
        Some(url) => url,
        None => {
            info.add_issue(
                ProjectIssue::warning("config", format!("{} is not set in .env", url_var_name))
                    .with_suggestion(format!("Set {} to your local development URL", url_var_name)),
            );
            return;
        }
    };

    // Find the domain for this project's directory
    let document_root = path.to_string_lossy().to_string();

    // First, check if there's an instance linked to this directory
    let instance = config.instances.iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == document_root)
            .unwrap_or(false)
    });

    // Find associated domain
    let domain = if let Some(inst) = instance {
        config.domains.iter().find(|d| d.routes_to_instance(&inst.id))
    } else {
        None
    };

    if let Some(domain) = domain {
        let expected_url = if domain.ssl_enabled {
            format!("https://{}.{}", domain.subdomain, config.tld)
        } else if config.proxy_installed {
            format!("http://{}.{}", domain.subdomain, config.tld)
        } else {
            format!("http://{}.{}:{}", domain.subdomain, config.tld, config.proxy_port)
        };

        // Normalize URLs for comparison (remove trailing slashes)
        let site_url_normalized = site_url.trim_end_matches('/');
        let expected_normalized = expected_url.trim_end_matches('/');

        if !site_url_normalized.eq_ignore_ascii_case(expected_normalized) {
            info.add_issue(
                ProjectIssue::warning(
                    "config",
                    format!(
                        "{} '{}' doesn't match Burd domain '{}'",
                        url_var_name, site_url, expected_url
                    ),
                )
                .with_suggestion(format!("Update {} to {} in .env", url_var_name, expected_url)),
            );
        }
    } else {
        // No domain linked - check if URL looks like a Burd domain
        let burd_domain_suffix = format!(".{}", config.tld);
        if !site_url.contains(&burd_domain_suffix) {
            info.add_issue(
                ProjectIssue::info(
                    "config",
                    format!("{} '{}' is not a Burd domain", url_var_name, site_url),
                )
                .with_suggestion(format!(
                    "Use 'burd link' to create a .{} domain for this project",
                    config.tld
                )),
            );
        }
    }
}

/// Check if instance document_root matches the computed document_root for the project type
fn check_document_root_config(info: &mut ProjectInfo, path: &Path, config: &Config) {
    let computed_doc_root = info.document_root.to_string_lossy().to_string();
    let project_root = path.to_string_lossy().to_string();

    // Only check if computed document root differs from project root
    // (e.g., Laravel needs /public, Bedrock needs /web)
    if computed_doc_root == project_root {
        return;
    }

    // Find instance linked to this directory (either pointing to project root or computed root)
    let instance = config.instances.iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == project_root || dr == computed_doc_root)
            .unwrap_or(false)
    });

    if let Some(inst) = instance {
        let inst_doc_root = inst
            .config
            .get("document_root")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Check if instance points to project root instead of computed document root
        if inst_doc_root == project_root {
            info.add_issue(
                ProjectIssue::warning(
                    "config",
                    format!(
                        "Instance '{}' points to '{}' but {} projects need '{}'",
                        inst.name, project_root, info.project_type, computed_doc_root
                    ),
                )
                .with_suggestion(format!(
                    "Run 'burd unlink && burd link' to fix the document root to '{}'",
                    computed_doc_root
                )),
            );
        }
    }
}

/// Find a database instance in Burd config that matches the connection type
pub fn find_matching_db_instance<'a>(
    config: &'a Config,
    connection_type: &str,
) -> Option<&'a Instance> {
    config.instances.iter().find(|i| {
        match connection_type {
            "mysql" | "mariadb" => i.service_type == ServiceType::MariaDB,
            "pgsql" | "postgres" | "postgresql" => i.service_type == ServiceType::PostgreSQL,
            _ => false,
        }
    })
}

/// Get all database instances from Burd config
pub fn get_db_instances(config: &Config) -> Vec<&Instance> {
    config
        .instances
        .iter()
        .filter(|i| {
            matches!(
                i.service_type,
                ServiceType::MariaDB | ServiceType::PostgreSQL
            )
        })
        .collect()
}
