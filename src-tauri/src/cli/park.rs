//! Park CLI commands
//!
//! Commands for managing parked directories from the command line.

use crate::caddy;
use crate::cli::init::is_initialized;
use crate::config::ConfigStore;
use crate::park::{self, generate_subdomain};
use std::env;
use std::path::Path;

/// Park the current directory
///
/// Adds the current directory to the list of parked directories.
/// All subdirectories will automatically become domains.
pub fn run_park() -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let path = current_dir.to_string_lossy().to_string();

    println!("Parking directory: {}", path);

    let config_store = ConfigStore::new()?;

    // Check if park is enabled (FrankenPHP Park instance exists)
    if !config_store.is_park_enabled()? {
        return Err(
            "FrankenPHP Park is not enabled.\n\
             Please create a FrankenPHP Park instance in the Burd app first:\n\
             1. Open Burd\n\
             2. Go to Instances\n\
             3. Create a new 'FrankenPHP Park' instance"
                .to_string(),
        );
    }

    // Check if already parked
    if config_store.find_parked_directory_by_path(&path)?.is_some() {
        return Err(format!("Directory '{}' is already parked", path));
    }

    // Create the parked directory
    let _parked_dir = config_store.create_parked_directory(path.clone(), true)?;

    // Scan for projects
    let projects = park::scan_directory(Path::new(&path)).unwrap_or_default();

    let config = config_store.load()?;

    println!();
    println!("Parked '{}' successfully!", path);
    println!();

    if projects.is_empty() {
        println!("No projects found yet. Add subdirectories to create domains.");
    } else {
        println!("Found {} project(s):", projects.len());
        for project in &projects {
            let subdomain = generate_subdomain(&project.name);
            let domain = format!("{}.{}", subdomain, config.tld);
            let type_label = match project.project_type.as_str() {
                "php-laravel" => "Laravel",
                "php" => "PHP",
                "static" => "Static",
                _ => "Unknown",
            };
            println!("  {} -> {} ({})", project.name, domain, type_label);
        }
    }

    println!();
    println!("Note: Run 'burd parked' to see all parked directories.");
    println!("      Domains will be synced automatically when projects are added/removed.");

    Ok(())
}

/// Unpark (forget) the current directory
///
/// Removes the current directory from the list of parked directories.
pub fn run_forget() -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let path = current_dir.to_string_lossy().to_string();

    let config_store = ConfigStore::new()?;

    // Find the parked directory
    let parked_dir = config_store
        .find_parked_directory_by_path(&path)?
        .ok_or_else(|| format!("Directory '{}' is not parked", path))?;

    // Get domains that will be removed
    let domains = config_store.get_domains_for_parked_directory(parked_dir.id)?;
    let domain_count = domains.len();
    let config = config_store.load()?;

    // Delete the .caddy files for each domain
    for domain in &domains {
        let full_domain = format!("{}.{}", domain.subdomain, config.tld);
        if let Err(e) = caddy::delete_domain_file(&full_domain) {
            eprintln!("Warning: Failed to delete domain file for {}: {}", full_domain, e);
        }
    }

    // Delete domains and parked directory from database
    config_store.delete_domains_for_parked_directory(parked_dir.id)?;
    config_store.delete_parked_directory(parked_dir.id)?;

    // Regenerate park caddyfile if park instance exists
    if let Some(park_instance) = config_store.get_park_instance()? {
        let _ = park::regenerate_park_caddyfile(&config_store, &park_instance, &config.tld);
    }

    println!();
    println!("Unparked '{}' successfully!", path);
    if domain_count > 0 {
        println!("Removed {} domain(s).", domain_count);
    }
    println!();

    Ok(())
}

/// List all parked directories
pub fn run_parked() -> Result<(), String> {
    let config_store = ConfigStore::new()?;

    // Check if park is enabled
    if !config_store.is_park_enabled()? {
        println!("FrankenPHP Park is not enabled.");
        println!();
        println!("Create a FrankenPHP Park instance in the Burd app to enable parking.");
        return Ok(());
    }

    let parked_dirs = config_store.list_parked_directories()?;
    let config = config_store.load()?;

    if parked_dirs.is_empty() {
        println!("No directories are currently parked.");
        println!();
        println!("Park a directory with: burd park");
        return Ok(());
    }

    println!("Parked Directories:");
    println!();

    for dir in parked_dirs {
        let projects = park::scan_directory(Path::new(&dir.path)).unwrap_or_default();
        let ssl_status = if dir.ssl_enabled { "SSL" } else { "HTTP" };

        println!("  {} ({} projects, {})", dir.path, projects.len(), ssl_status);

        // Show projects
        for project in projects {
            let subdomain = generate_subdomain(&project.name);
            let domain = format!("{}.{}", subdomain, config.tld);
            let type_label = match project.project_type.as_str() {
                "php-laravel" => "Laravel",
                "php" => "PHP",
                "static" => "Static",
                _ => "Unknown",
            };
            println!("    {} -> {} ({})", project.name, domain, type_label);
        }
        println!();
    }

    Ok(())
}

/// Refresh all parked directories (sync domains)
pub fn run_refresh() -> Result<(), String> {
    let config_store = ConfigStore::new()?;

    // Check if park is enabled
    if !config_store.is_park_enabled()? {
        return Err("FrankenPHP Park is not enabled.".to_string());
    }

    let parked_dirs = config_store.list_parked_directories()?;
    let config = config_store.load()?;

    if parked_dirs.is_empty() {
        println!("No directories are currently parked.");
        return Ok(());
    }

    println!("Refreshing parked directories...");
    println!();

    // We need a proxy server for sync, but for CLI we'll just update config
    // The actual proxy sync happens when the app is running
    let mut total_added = 0;
    let mut total_removed = 0;

    for dir in parked_dirs {
        // For CLI, we just report what would be synced
        let projects = park::scan_directory(Path::new(&dir.path)).unwrap_or_default();

        // Check for new projects
        let existing_domains: std::collections::HashSet<String> = config
            .domains
            .iter()
            .filter(|d| d.parked_dir_id() == Some(dir.id))
            .map(|d| d.subdomain.clone())
            .collect();

        let discovered: std::collections::HashSet<String> = projects
            .iter()
            .map(|p| generate_subdomain(&p.name))
            .collect();

        let new_count = discovered.difference(&existing_domains).count();
        let removed_count = existing_domains.difference(&discovered).count();

        total_added += new_count;
        total_removed += removed_count;

        if new_count > 0 || removed_count > 0 {
            println!("  {}: +{} -{}", dir.path, new_count, removed_count);
        } else {
            println!("  {}: no changes", dir.path);
        }
    }

    println!();
    if total_added > 0 || total_removed > 0 {
        println!(
            "Summary: {} to add, {} to remove",
            total_added, total_removed
        );
        println!();
        println!("Note: Full sync requires the Burd app to be running.");
        println!("      Open Burd and go to Parks to sync changes.");
    } else {
        println!("All parked directories are up to date.");
    }

    Ok(())
}

/// Show park status for the current directory
pub fn run_status() -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let path = current_dir.to_string_lossy().to_string();

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Check if this directory is initialized with burd init
    if let Ok(Some(instance)) = is_initialized() {
        let domain = instance
            .domain
            .as_ref()
            .map(|d| format!("{}.{}", d, config.tld))
            .unwrap_or_else(|| "none".to_string());

        println!("Directory '{}' is initialized", path);
        println!();
        println!(
            "Instance: {} (FrankenPHP on port {})",
            instance.name, instance.port
        );
        println!("Domain: {}", domain);
        println!(
            "Domain enabled: {}",
            if instance.domain_enabled { "yes" } else { "no" }
        );
        println!();
        println!(
            "Start with: Open Burd app and click Start on '{}'",
            instance.name
        );

        return Ok(());
    }

    // Check if this directory is parked
    if let Some(parked_dir) = config_store.find_parked_directory_by_path(&path)? {
        let projects = park::scan_directory(Path::new(&path)).unwrap_or_default();
        let ssl_status = if parked_dir.ssl_enabled { "SSL enabled" } else { "SSL disabled" };

        println!("Directory '{}' is parked", path);
        println!();
        println!("Status: {} | {} project(s)", ssl_status, projects.len());
        println!();

        if !projects.is_empty() {
            println!("Projects:");
            for project in projects {
                let subdomain = generate_subdomain(&project.name);
                let domain = format!("{}.{}", subdomain, config.tld);
                let type_label = match project.project_type.as_str() {
                    "php-laravel" => "Laravel",
                    "php" => "PHP",
                    "static" => "Static",
                    _ => "Unknown",
                };
                println!("  {} -> {} ({})", project.name, domain, type_label);
            }
        }

        return Ok(());
    }

    // Check if current directory is inside a parked directory
    for parked_dir in config_store.list_parked_directories()? {
        if path.starts_with(&parked_dir.path) && path != parked_dir.path {
            // We're inside a parked directory - this might be a project
            let project_name = current_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            let subdomain = generate_subdomain(project_name);
            let domain = format!("{}.{}", subdomain, config.tld);

            // Detect project type
            let project_type = park::detect_project_type(&current_dir);
            let type_label = match project_type.as_str() {
                "php-laravel" => "Laravel",
                "php" => "PHP",
                "static" => "Static",
                _ => "Unknown",
            };

            println!("This directory is a project inside a parked directory.");
            println!();
            println!("  Parent: {}", parked_dir.path);
            println!("  Project: {}", project_name);
            println!("  Type: {}", type_label);
            println!("  Domain: {}", domain);

            return Ok(());
        }
    }

    println!("Directory '{}' is not initialized or parked.", path);
    println!();
    println!("Initialize with: burd init   (single project)");
    println!("Park with:       burd park   (multiple projects)");

    Ok(())
}
