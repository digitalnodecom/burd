//! MySQL/MariaDB CLI passthrough
//!
//! Runs MySQL/MariaDB tools with auto-connection to running Burd instances.

use crate::config::{get_versioned_binary_dir, ConfigStore, ServiceType};
use std::process::Command;

/// Run a MySQL/MariaDB tool with auto-connection
///
/// Finds the running MariaDB/MySQL instance, resolves the versioned binary path,
/// injects connection arguments, and executes the tool with any additional args.
pub fn run_mysql(tool: &str, args: Vec<String>) -> Result<(), String> {
    // Load config to find running instances
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Find a MySQL or MariaDB instance
    let instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::MariaDB || i.service_type == ServiceType::MySQL)
        .ok_or_else(|| {
            "No MySQL or MariaDB instance found.\n\
             Create one in the Burd app first."
                .to_string()
        })?;

    // Get the version - use instance version or find installed version
    let version = if instance.version.is_empty() {
        // Find any installed version for this service type
        config
            .binaries
            .get(&instance.service_type)
            .and_then(|versions| versions.keys().next())
            .cloned()
            .ok_or_else(|| {
                format!(
                    "No {} binary installed. Download one in the Burd app first.",
                    instance.service_type.display_name()
                )
            })?
    } else {
        instance.version.clone()
    };

    // Get the versioned binary directory
    let bin_dir = get_versioned_binary_dir(instance.service_type, &version)?;

    // Check if the tool exists in the bin directory
    let tool_path = bin_dir.join("bin").join(tool);
    if !tool_path.exists() {
        // Try without "bin" subdirectory (some packages have flat structure)
        let tool_path_flat = bin_dir.join(tool);
        if !tool_path_flat.exists() {
            return Err(format!(
                "Tool '{}' not found in {} {}\nAvailable at: {}",
                tool,
                instance.service_type.display_name(),
                version,
                bin_dir.display()
            ));
        }
    }

    let tool_path = if bin_dir.join("bin").join(tool).exists() {
        bin_dir.join("bin").join(tool)
    } else {
        bin_dir.join(tool)
    };

    // Build connection arguments based on instance config
    let mut connection_args = build_connection_args(instance);

    // Check if user provided any connection args - if so, don't inject ours
    let user_has_connection = args.iter().any(|a| {
        a.starts_with("--host")
            || a.starts_with("-h")
            || a.starts_with("--port")
            || a.starts_with("-P")
            || a.starts_with("--socket")
            || a.starts_with("-S")
            || a.starts_with("--user")
            || a.starts_with("-u")
    });

    // Build the final argument list
    let mut final_args: Vec<String> = Vec::new();
    if !user_has_connection {
        final_args.append(&mut connection_args);
    }
    final_args.extend(args);

    // Set up library path for bundled dylibs
    let lib_dir = bin_dir.join("lib");
    let dyld_path = if lib_dir.exists() {
        lib_dir.to_string_lossy().to_string()
    } else {
        String::new()
    };

    // Execute the tool
    let status = Command::new(&tool_path)
        .args(&final_args)
        .env("DYLD_LIBRARY_PATH", &dyld_path)
        .env("DYLD_FALLBACK_LIBRARY_PATH", &dyld_path)
        .status()
        .map_err(|e| format!("Failed to execute {}: {}", tool, e))?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

/// Build connection arguments for MySQL/MariaDB
fn build_connection_args(instance: &crate::config::Instance) -> Vec<String> {
    let mut args = Vec::new();

    // Check for socket first (preferred for local connections)
    if let Some(socket) = instance.config.get("socket").and_then(|v| v.as_str()) {
        if !socket.is_empty() {
            args.push(format!("--socket={}", socket));
        }
    } else {
        // Fall back to host/port
        args.push("--host=127.0.0.1".to_string());
        args.push(format!("--port={}", instance.port));
    }

    // Add user
    let user = instance
        .config
        .get("user")
        .and_then(|v| v.as_str())
        .unwrap_or("root");
    args.push(format!("--user={}", user));

    // Add password if set
    if let Some(password) = instance.config.get("password").and_then(|v| v.as_str()) {
        if !password.is_empty() {
            args.push(format!("--password={}", password));
        }
    }

    args
}

/// List available MySQL/MariaDB tools
pub fn list_mysql_tools() -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Check for MariaDB first, then MySQL
    let service_type = if config.binaries.contains_key(&ServiceType::MariaDB) {
        ServiceType::MariaDB
    } else if config.binaries.contains_key(&ServiceType::MySQL) {
        ServiceType::MySQL
    } else {
        println!("No MySQL or MariaDB binary installed.");
        println!("Install one via the Burd app first.");
        return Ok(());
    };

    let versions = match config.binaries.get(&service_type) {
        Some(v) => v,
        None => {
            println!("No {} binaries found.", service_type.display_name());
            return Ok(());
        }
    };

    let version = match versions.keys().next() {
        Some(v) => v,
        None => {
            println!("No {} versions installed.", service_type.display_name());
            return Ok(());
        }
    };

    let bin_dir = get_versioned_binary_dir(service_type, version)?;
    let tools_dir = bin_dir.join("bin");

    if !tools_dir.exists() {
        println!("Binary directory not found: {}", tools_dir.display());
        return Ok(());
    }

    println!("Available {} {} tools:", service_type.display_name(), version);
    println!();

    let mut tools: Vec<String> = std::fs::read_dir(&tools_dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_type()
                .map(|ft| ft.is_file() || ft.is_symlink())
                .unwrap_or(false)
        })
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();

    tools.sort();

    for tool in tools {
        println!("  {}", tool);
    }

    Ok(())
}
