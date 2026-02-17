//! Setup CLI command
//!
//! Full interactive project setup wizard that handles:
//! - Project analysis
//! - FrankenPHP instance creation
//! - Database setup
//! - Cache configuration (Redis)
//! - Mail configuration (Mailpit)
//! - Migrations (Laravel)

use crate::analyzer::{
    analyze_project, extract_cache_config, extract_database_config, extract_mail_config,
    parse_env_file, update_env_value, ProjectType,
};
use crate::config::{ConfigStore, Domain, Instance, ServiceType};
use crate::db_manager::{create_manager_for_instance, find_all_db_instances, sanitize_db_name};
use chrono::Utc;
use std::env;
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

/// Run the full project setup wizard
///
/// Interactive wizard that sets up everything needed for a project:
/// - Analyzes the project type
/// - Creates FrankenPHP instance and domain
/// - Sets up database
/// - Configures Redis for cache/sessions
/// - Configures Mailpit for local mail
/// - Runs migrations (for Laravel)
pub fn run_setup() -> Result<(), String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;

    let project_name = current_dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Could not determine project name from directory".to_string())?
        .to_string();

    println!();
    println!("Burd Project Setup");
    println!("==================");
    println!();
    println!("Directory: {}", current_dir.display());

    // Analyze the project
    println!();
    println!("Analyzing project...");

    let project =
        analyze_project(&current_dir).map_err(|e| format!("Failed to analyze project: {}", e))?;

    if matches!(project.project_type, ProjectType::Unknown) {
        return Err("Could not detect project type.\n\
             Supported: Laravel, WordPress, Bedrock, Symfony\n\n\
             Make sure you're in a project directory with:\n  \
             - artisan (Laravel)\n  \
             - wp-config.php (WordPress)\n  \
             - web/wp/ directory (Bedrock)"
            .to_string());
    }

    println!("Detected: {}", project.project_type);

    let config_store = ConfigStore::new()?;

    // Track what we set up
    let mut setup_steps: Vec<String> = Vec::new();

    // Step 1: FrankenPHP Instance
    println!();
    println!("[1/5] FrankenPHP Instance");
    println!("-------------------------");

    let instance_created = setup_frankenphp_instance(&current_dir, &project_name, &config_store)?;

    if instance_created {
        setup_steps.push(format!("Created FrankenPHP instance '{}'", project_name));
    }

    // Reload config after instance creation
    let config = config_store.load()?;

    // Step 2: Database
    println!();
    println!("[2/5] Database");
    println!("--------------");

    let db_created = setup_database(&current_dir, &project, &config)?;
    if let Some(db_name) = db_created {
        setup_steps.push(format!("Created database '{}'", db_name));
    }

    // Step 3: Cache (Redis)
    println!();
    println!("[3/5] Cache (Redis)");
    println!("-------------------");

    let cache_configured = setup_cache(&current_dir, &project, &config)?;
    if cache_configured {
        setup_steps.push("Configured Redis for cache/sessions".to_string());
    }

    // Step 4: Mail (Mailpit)
    println!();
    println!("[4/5] Mail (Mailpit)");
    println!("--------------------");

    let mail_configured = setup_mail(&current_dir, &project, &config)?;
    if mail_configured {
        setup_steps.push("Configured Mailpit for local mail".to_string());
    }

    // Step 5: Migrations (Laravel only)
    println!();
    println!("[5/5] Migrations");
    println!("----------------");

    let migrations_run = setup_migrations(&current_dir, &project)?;
    if migrations_run {
        setup_steps.push("Ran database migrations".to_string());
    }

    // Summary
    println!();
    println!("Setup Complete!");
    println!("===============");
    println!();

    if setup_steps.is_empty() {
        println!("No changes were made. Project was already configured.");
    } else {
        println!("What was done:");
        for step in &setup_steps {
            println!("  - {}", step);
        }
    }

    // Show access info
    let config = config_store.load()?;
    let subdomain = slug::slugify(&project_name);

    if let Some(domain) = config.domains.iter().find(|d| d.subdomain == subdomain) {
        println!();
        println!("Access your project:");
        let url = if config.proxy_installed {
            format!("https://{}.{}", domain.subdomain, config.tld)
        } else {
            format!(
                "http://{}.{}:{}",
                domain.subdomain, config.tld, config.proxy_port
            )
        };
        println!("  URL: {}", url);
    }

    // Show Mailpit if configured
    if let Some(mailpit) = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Mailpit)
    {
        let web_port = mailpit
            .config
            .get("web_port")
            .and_then(|v| v.as_u64())
            .unwrap_or(8025);
        println!("  Mail: http://localhost:{}", web_port);
    }

    println!();
    println!("Start the server in the Burd app to begin development.");

    Ok(())
}

/// Set up FrankenPHP instance for the project
fn setup_frankenphp_instance(
    project_dir: &Path,
    project_name: &str,
    config_store: &ConfigStore,
) -> Result<bool, String> {
    let mut config = config_store.load()?;
    let document_root = project_dir.to_string_lossy().to_string();
    let subdomain = slug::slugify(project_name);

    // Check if already linked
    let existing = config.instances.iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == document_root)
            .unwrap_or(false)
    });

    if existing.is_some() {
        println!("Already linked to {}.{}", subdomain, config.tld);
        return Ok(false);
    }

    // Check if subdomain already exists
    if config.domains.iter().any(|d| d.subdomain == subdomain) {
        println!("Domain {}.{} already exists.", subdomain, config.tld);
        return Ok(false);
    }

    print!(
        "Create instance '{}' on {}.{}? [Y/n] ",
        project_name, subdomain, config.tld
    );
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("Skipped.");
        return Ok(false);
    }

    // Find available port
    let mut port = ServiceType::FrankenPHP.default_port();
    while config.instances.iter().any(|i| i.port == port) {
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

    // Create instance
    let instance = Instance {
        id: Uuid::new_v4(),
        name: project_name.to_string(),
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

    // Create instance directory
    let instance_dir = crate::config::get_instance_dir(&instance.id)?;
    std::fs::create_dir_all(&instance_dir)
        .map_err(|e| format!("Failed to create instance directory: {}", e))?;

    config.instances.push(instance.clone());

    // Create domain
    let domain = Domain::for_instance(subdomain.clone(), instance.id, false);
    config.domains.push(domain);

    config_store.save(&config)?;

    println!(
        "Created instance '{}' -> {}.{}",
        project_name, subdomain, config.tld
    );
    Ok(true)
}

/// Set up database for the project
fn setup_database(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
) -> Result<Option<String>, String> {
    // Skip if project doesn't use databases
    if !project.project_type.uses_env_file() && !project.project_type.uses_wp_config() {
        println!("Project type doesn't use a database configuration.");
        return Ok(None);
    }

    // Skip SQLite
    if let Some(ref db) = project.database {
        if db.is_sqlite() {
            println!("Project uses SQLite (no server needed).");
            return Ok(None);
        }
    }

    // Find database instances
    let db_instances = find_all_db_instances(config);
    if db_instances.is_empty() {
        println!("No database service configured in Burd.");
        println!("Add MariaDB or PostgreSQL in the Burd app first.");
        return Ok(None);
    }

    // Get database name
    let db_name = project
        .database
        .as_ref()
        .filter(|d| !d.database.is_empty() && !d.is_sqlite())
        .map(|d| d.database.clone())
        .unwrap_or_else(|| {
            sanitize_db_name(&project.name).unwrap_or_else(|_| project.name.clone())
        });

    let db_instance = db_instances[0];
    let manager = create_manager_for_instance(db_instance)?;

    // Check if exists
    let exists = manager.database_exists(&db_name).unwrap_or(false);

    if exists {
        println!(
            "Database '{}' already exists on {:?}.",
            db_name, db_instance.service_type
        );

        // Still offer to fix .env if port doesn't match
        fix_database_env(project_dir, project, db_instance)?;
        return Ok(None);
    }

    print!(
        "Create database '{}' on {:?} (port {})? [Y/n] ",
        db_name, db_instance.service_type, db_instance.port
    );
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("Skipped.");
        return Ok(None);
    }

    manager.create_database(&db_name)?;
    println!("Created database '{}'.", db_name);

    // Fix .env to point to this database
    fix_database_env(project_dir, project, db_instance)?;

    Ok(Some(db_name))
}

/// Fix database configuration in .env
fn fix_database_env(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    db_instance: &Instance,
) -> Result<(), String> {
    if !project.project_type.uses_env_file() {
        return Ok(());
    }

    let env_path = project_dir.join(".env");
    if !env_path.exists() {
        return Ok(());
    }

    let env_vars = match parse_env_file(&env_path) {
        Some(vars) => vars,
        None => return Ok(()),
    };

    let db_config = extract_database_config(&project.project_type, &env_vars);

    if let Some(db) = db_config {
        let mut fixes: Vec<(&str, String)> = Vec::new();

        if db.port != db_instance.port {
            fixes.push(("DB_PORT", db_instance.port.to_string()));
        }

        if db.host != "127.0.0.1" && db.host != "localhost" {
            fixes.push(("DB_HOST", "127.0.0.1".to_string()));
        }

        if !fixes.is_empty() {
            println!();
            println!("Update .env to use Burd's {:?}?", db_instance.service_type);
            for (key, value) in &fixes {
                println!("  {} = {}", key, value);
            }

            print!("Apply? [Y/n] ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| format!("Failed to read input: {}", e))?;

            if !input.trim().eq_ignore_ascii_case("n") {
                for (key, value) in fixes {
                    update_env_value(&env_path, key, &value)?;
                }
                println!("Updated .env");
            }
        }
    }

    Ok(())
}

/// Set up Redis cache configuration
fn setup_cache(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
) -> Result<bool, String> {
    // Only for Laravel
    if !matches!(project.project_type, ProjectType::Laravel { .. }) {
        println!("Cache configuration is Laravel-specific. Skipping.");
        return Ok(false);
    }

    let env_path = project_dir.join(".env");
    if !env_path.exists() {
        println!("No .env file found.");
        return Ok(false);
    }

    // Find Redis instance
    let redis_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Redis);

    let redis_instance = match redis_instance {
        Some(i) => i,
        None => {
            println!("No Redis instance configured in Burd.");
            return Ok(false);
        }
    };

    let env_vars = match parse_env_file(&env_path) {
        Some(vars) => vars,
        None => return Ok(false),
    };

    let cache_config = extract_cache_config(&project.project_type, &env_vars);

    // Check current configuration
    let current_driver = env_vars.get("CACHE_STORE").or(env_vars.get("CACHE_DRIVER"));
    let is_redis = current_driver.map(|d| d == "redis").unwrap_or(false);

    if is_redis {
        // Already using Redis, check port
        if let Some(cache) = cache_config {
            if cache.port == Some(redis_instance.port) {
                println!(
                    "Already configured for Redis on port {}.",
                    redis_instance.port
                );
                return Ok(false);
            }
        }
    }

    print!(
        "Configure Redis for cache/sessions (port {})? [Y/n] ",
        redis_instance.port
    );
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("Skipped.");
        return Ok(false);
    }

    // Update .env
    let updates = [
        ("CACHE_STORE", "redis"),
        ("SESSION_DRIVER", "redis"),
        ("REDIS_HOST", "127.0.0.1"),
        ("REDIS_PORT", &redis_instance.port.to_string()),
    ];

    for (key, value) in &updates {
        update_env_value(&env_path, key, value)?;
    }

    println!("Updated CACHE_STORE, SESSION_DRIVER, REDIS_HOST, REDIS_PORT");
    Ok(true)
}

/// Set up Mailpit mail configuration
fn setup_mail(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
    config: &crate::config::Config,
) -> Result<bool, String> {
    // Only for Laravel
    if !matches!(project.project_type, ProjectType::Laravel { .. }) {
        println!("Mail configuration is Laravel-specific. Skipping.");
        return Ok(false);
    }

    let env_path = project_dir.join(".env");
    if !env_path.exists() {
        println!("No .env file found.");
        return Ok(false);
    }

    // Find Mailpit instance
    let mailpit_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Mailpit);

    let mailpit_instance = match mailpit_instance {
        Some(i) => i,
        None => {
            println!("No Mailpit instance configured in Burd.");
            return Ok(false);
        }
    };

    let smtp_port = mailpit_instance
        .config
        .get("smtp_port")
        .and_then(|v| v.as_u64())
        .unwrap_or(1025) as u16;

    let env_vars = match parse_env_file(&env_path) {
        Some(vars) => vars,
        None => return Ok(false),
    };

    let mail_config = extract_mail_config(&project.project_type, &env_vars);

    // Check if already configured for Mailpit
    if let Some(mail) = mail_config {
        if mail.host == "127.0.0.1" && mail.port == smtp_port {
            println!("Already configured for Mailpit on port {}.", smtp_port);
            return Ok(false);
        }
    }

    let web_port = mailpit_instance
        .config
        .get("web_port")
        .and_then(|v| v.as_u64())
        .unwrap_or(8025);

    print!(
        "Configure Mailpit for local mail (SMTP {}, Web {})? [Y/n] ",
        smtp_port, web_port
    );
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("Skipped.");
        return Ok(false);
    }

    // Update .env
    let updates = [
        ("MAIL_MAILER", "smtp"),
        ("MAIL_HOST", "127.0.0.1"),
        ("MAIL_PORT", &smtp_port.to_string()),
        ("MAIL_USERNAME", ""),
        ("MAIL_PASSWORD", ""),
        ("MAIL_ENCRYPTION", ""),
    ];

    for (key, value) in &updates {
        update_env_value(&env_path, key, value)?;
    }

    println!("Updated MAIL_MAILER, MAIL_HOST, MAIL_PORT");
    Ok(true)
}

/// Run database migrations (Laravel only)
fn setup_migrations(
    project_dir: &Path,
    project: &crate::analyzer::ProjectInfo,
) -> Result<bool, String> {
    // Only for Laravel
    if !matches!(project.project_type, ProjectType::Laravel { .. }) {
        println!("Migrations are Laravel-specific. Skipping.");
        return Ok(false);
    }

    // Check if artisan exists
    let artisan_path = project_dir.join("artisan");
    if !artisan_path.exists() {
        println!("No artisan file found.");
        return Ok(false);
    }

    print!("Run database migrations? [y/N] ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    if !input.trim().eq_ignore_ascii_case("y") {
        println!("Skipped.");
        return Ok(false);
    }

    println!("Running: php artisan migrate");

    let status = Command::new("php")
        .args(["artisan", "migrate", "--force"])
        .current_dir(project_dir)
        .status()
        .map_err(|e| format!("Failed to run migrations: {}", e))?;

    if !status.success() {
        eprintln!("Warning: Migrations failed. You may need to run them manually.");
        return Ok(false);
    }

    println!("Migrations completed.");
    Ok(true)
}
