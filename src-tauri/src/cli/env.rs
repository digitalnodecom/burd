//! Environment CLI commands
//!
//! Commands for checking and fixing .env files against Burd services.

use crate::analyzer::{
    analyze_with_burd_config, extract_cache_config, extract_database_config, extract_mail_config,
    parse_env_file, update_env_value, ProjectType,
};
use crate::config::{ConfigStore, ServiceType};
use std::collections::HashMap;
use std::env;
use std::io::{self, Write};

/// An issue found in the .env file
#[derive(Debug, Clone)]
pub struct EnvIssue {
    /// The environment variable key
    pub key: String,
    /// Current value in .env
    pub current: String,
    /// Suggested value
    pub suggested: String,
    /// Reason for the suggestion
    pub reason: String,
    /// Category (database, cache, mail, etc.)
    pub category: String,
}

/// Check .env file against Burd services
pub fn run_env_check() -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Analyze the project
    let project = analyze_with_burd_config(&current_dir, &config)?;

    if !project.project_type.uses_env_file() {
        println!(
            "This project type ({}) doesn't use .env files.",
            project.project_type
        );
        return Ok(());
    }

    let env_path = current_dir.join(".env");
    if !env_path.exists() {
        return Err("No .env file found in current directory.".to_string());
    }

    let env_vars = parse_env_file(&env_path).ok_or("Failed to parse .env file")?;
    let issues = check_env_against_burd(&project.project_type, &env_vars, &config)?;

    print_env_check_results(&issues);

    Ok(())
}

/// Interactive fix for .env issues
pub fn run_env_fix() -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Analyze the project
    let project = analyze_with_burd_config(&current_dir, &config)?;

    if !project.project_type.uses_env_file() {
        println!(
            "This project type ({}) doesn't use .env files.",
            project.project_type
        );
        return Ok(());
    }

    let env_path = current_dir.join(".env");
    if !env_path.exists() {
        return Err("No .env file found in current directory.".to_string());
    }

    let env_vars = parse_env_file(&env_path).ok_or("Failed to parse .env file")?;
    let issues = check_env_against_burd(&project.project_type, &env_vars, &config)?;

    if issues.is_empty() {
        println!("No issues found. Your .env file is configured correctly for Burd services.");
        return Ok(());
    }

    println!();
    println!("Found {} issue(s) in .env file:", issues.len());
    println!();

    let mut fixed_count = 0;

    for issue in &issues {
        println!("[{}] {}", issue.category, issue.key);
        println!(
            "  Current:   {}",
            mask_sensitive(&issue.key, &issue.current)
        );
        println!(
            "  Suggested: {}",
            mask_sensitive(&issue.key, &issue.suggested)
        );
        println!("  Reason:    {}", issue.reason);
        println!();

        print!("Apply this fix? [y/N] ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {}", e))?;

        if input.trim().eq_ignore_ascii_case("y") {
            update_env_value(&env_path, &issue.key, &issue.suggested)?;
            println!("  Updated {}", issue.key);
            fixed_count += 1;
        } else {
            println!("  Skipped");
        }
        println!();
    }

    println!();
    println!("Fixed {} of {} issue(s).", fixed_count, issues.len());

    Ok(())
}

/// Show relevant .env values
pub fn run_env_show() -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Analyze the project
    let project = analyze_with_burd_config(&current_dir, &config)?;

    if !project.project_type.uses_env_file() {
        println!(
            "This project type ({}) doesn't use .env files.",
            project.project_type
        );
        return Ok(());
    }

    let env_path = current_dir.join(".env");
    if !env_path.exists() {
        return Err("No .env file found in current directory.".to_string());
    }

    let env_vars = parse_env_file(&env_path).ok_or("Failed to parse .env file")?;

    println!();
    println!("Environment: {}", project.project_type);
    println!("{}", "=".repeat(40));

    // Database settings
    println!();
    println!("Database:");
    print_env_var(&env_vars, "DB_CONNECTION", None);
    print_env_var(&env_vars, "DB_HOST", None);
    print_env_var(&env_vars, "DB_PORT", None);
    print_env_var(&env_vars, "DB_DATABASE", None);
    print_env_var(&env_vars, "DB_NAME", None); // Bedrock
    print_env_var(&env_vars, "DB_USERNAME", None);
    print_env_var(&env_vars, "DB_USER", None); // Bedrock
    print_env_var(&env_vars, "DB_PASSWORD", Some("********"));

    // Cache settings (Laravel)
    if matches!(project.project_type, ProjectType::Laravel { .. }) {
        println!();
        println!("Cache:");
        print_env_var(&env_vars, "CACHE_DRIVER", None);
        print_env_var(&env_vars, "CACHE_STORE", None);
        print_env_var(&env_vars, "SESSION_DRIVER", None);
        print_env_var(&env_vars, "REDIS_HOST", None);
        print_env_var(&env_vars, "REDIS_PORT", None);
        print_env_var(&env_vars, "REDIS_PASSWORD", Some("********"));
    }

    // Mail settings (Laravel)
    if matches!(project.project_type, ProjectType::Laravel { .. }) {
        println!();
        println!("Mail:");
        print_env_var(&env_vars, "MAIL_MAILER", None);
        print_env_var(&env_vars, "MAIL_HOST", None);
        print_env_var(&env_vars, "MAIL_PORT", None);
        print_env_var(&env_vars, "MAIL_USERNAME", None);
        print_env_var(&env_vars, "MAIL_PASSWORD", Some("********"));
    }

    // Search settings (Laravel)
    if matches!(project.project_type, ProjectType::Laravel { .. })
        && env_vars.contains_key("SCOUT_DRIVER")
    {
        println!();
        println!("Search:");
        print_env_var(&env_vars, "SCOUT_DRIVER", None);
        print_env_var(&env_vars, "MEILISEARCH_HOST", None);
        print_env_var(&env_vars, "MEILISEARCH_KEY", Some("********"));
    }

    println!();
    Ok(())
}

/// Check environment variables against Burd services
fn check_env_against_burd(
    project_type: &ProjectType,
    env_vars: &HashMap<String, String>,
    config: &crate::config::Config,
) -> Result<Vec<EnvIssue>, String> {
    let mut issues = Vec::new();

    // Check database configuration
    if let Some(db_config) = extract_database_config(project_type, env_vars) {
        check_database_env(&db_config, env_vars, config, &mut issues);
    }

    // Check cache configuration (Laravel only)
    if matches!(project_type, ProjectType::Laravel { .. }) {
        if let Some(cache_config) = extract_cache_config(project_type, env_vars) {
            check_cache_env(&cache_config, env_vars, config, &mut issues);
        }
    }

    // Check mail configuration (Laravel only)
    if matches!(project_type, ProjectType::Laravel { .. }) {
        if let Some(mail_config) = extract_mail_config(project_type, env_vars) {
            check_mail_env(&mail_config, env_vars, config, &mut issues);
        }
    }

    Ok(issues)
}

/// Check database environment variables
fn check_database_env(
    db_config: &crate::analyzer::DatabaseConfig,
    _env_vars: &HashMap<String, String>,
    config: &crate::config::Config,
    issues: &mut Vec<EnvIssue>,
) {
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

    let Some(instance) = db_instance else {
        return; // No matching database service
    };

    // Check if port matches
    if db_config.port != instance.port {
        let port_key = "DB_PORT";

        issues.push(EnvIssue {
            key: port_key.to_string(),
            current: db_config.port.to_string(),
            suggested: instance.port.to_string(),
            reason: format!(
                "Burd's {:?} is running on port {}",
                instance.service_type, instance.port
            ),
            category: "database".to_string(),
        });
    }

    // Check if host is localhost/127.0.0.1
    if db_config.host != "127.0.0.1" && db_config.host != "localhost" {
        let host_key = "DB_HOST";

        issues.push(EnvIssue {
            key: host_key.to_string(),
            current: db_config.host.clone(),
            suggested: "127.0.0.1".to_string(),
            reason: "Burd services run locally".to_string(),
            category: "database".to_string(),
        });
    }
}

/// Check cache environment variables
fn check_cache_env(
    cache_config: &crate::analyzer::CacheConfig,
    _env_vars: &HashMap<String, String>,
    config: &crate::config::Config,
    issues: &mut Vec<EnvIssue>,
) {
    if !cache_config.is_redis() {
        return;
    }

    // Find Redis instance
    let redis_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Redis);

    let Some(instance) = redis_instance else {
        return;
    };

    // Check port
    if let Some(port) = cache_config.port {
        if port != instance.port {
            issues.push(EnvIssue {
                key: "REDIS_PORT".to_string(),
                current: port.to_string(),
                suggested: instance.port.to_string(),
                reason: format!("Burd's Redis is running on port {}", instance.port),
                category: "cache".to_string(),
            });
        }
    }

    // Check host
    if let Some(ref host) = cache_config.host {
        if host != "127.0.0.1" && host != "localhost" {
            issues.push(EnvIssue {
                key: "REDIS_HOST".to_string(),
                current: host.clone(),
                suggested: "127.0.0.1".to_string(),
                reason: "Burd's Redis runs locally".to_string(),
                category: "cache".to_string(),
            });
        }
    }
}

/// Check mail environment variables
fn check_mail_env(
    mail_config: &crate::analyzer::MailConfig,
    _env_vars: &HashMap<String, String>,
    config: &crate::config::Config,
    issues: &mut Vec<EnvIssue>,
) {
    // Find Mailpit instance
    let mailpit_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Mailpit);

    let Some(instance) = mailpit_instance else {
        return;
    };

    // Mailpit SMTP port is typically 1025
    let smtp_port = instance
        .config
        .get("smtp_port")
        .and_then(|v| v.as_u64())
        .unwrap_or(1025) as u16;

    // Check if using SMTP
    if mail_config.mailer != "smtp" {
        // Suggest switching to SMTP for local development
        if mail_config.mailer == "log" || mail_config.mailer == "array" {
            // These are fine for development, don't suggest change
            return;
        }
    }

    // Check port
    if mail_config.mailer == "smtp" && mail_config.port != smtp_port {
        issues.push(EnvIssue {
            key: "MAIL_PORT".to_string(),
            current: mail_config.port.to_string(),
            suggested: smtp_port.to_string(),
            reason: format!("Burd's Mailpit SMTP is on port {}", smtp_port),
            category: "mail".to_string(),
        });
    }

    // Check host
    if mail_config.mailer == "smtp"
        && mail_config.host != "127.0.0.1"
        && mail_config.host != "localhost"
    {
        issues.push(EnvIssue {
            key: "MAIL_HOST".to_string(),
            current: mail_config.host.clone(),
            suggested: "127.0.0.1".to_string(),
            reason: "Burd's Mailpit runs locally".to_string(),
            category: "mail".to_string(),
        });
    }
}

/// Print environment check results
fn print_env_check_results(issues: &[EnvIssue]) {
    println!();

    if issues.is_empty() {
        println!("No issues found. Your .env file is configured correctly for Burd services.");
        println!();
        return;
    }

    println!("Found {} issue(s):", issues.len());
    println!();

    // Group by category
    let mut by_category: HashMap<&str, Vec<&EnvIssue>> = HashMap::new();
    for issue in issues {
        by_category.entry(&issue.category).or_default().push(issue);
    }

    for (category, cat_issues) in by_category {
        println!("[{}]", category);
        for issue in cat_issues {
            println!("  {} = {} -> {}", issue.key, issue.current, issue.suggested);
            println!("    {}", issue.reason);
        }
        println!();
    }

    println!("Run 'burd env fix' to fix these issues interactively.");
    println!();
}

/// Print an environment variable if it exists
fn print_env_var(env_vars: &HashMap<String, String>, key: &str, mask: Option<&str>) {
    if let Some(value) = env_vars.get(key) {
        let display_value = if let Some(m) = mask {
            if value.is_empty() {
                "(empty)".to_string()
            } else {
                m.to_string()
            }
        } else {
            value.clone()
        };
        println!("  {} = {}", key, display_value);
    }
}

/// Mask sensitive values for display
fn mask_sensitive(key: &str, value: &str) -> String {
    let sensitive_keys = ["PASSWORD", "SECRET", "KEY", "TOKEN"];
    let is_sensitive = sensitive_keys
        .iter()
        .any(|k| key.to_uppercase().contains(k));

    if is_sensitive && !value.is_empty() {
        "********".to_string()
    } else if value.is_empty() {
        "(empty)".to_string()
    } else {
        value.to_string()
    }
}
