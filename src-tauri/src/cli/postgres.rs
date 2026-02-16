//! PostgreSQL CLI passthrough
//!
//! Runs PostgreSQL tools with auto-connection to running Burd instances.

use crate::config::{get_versioned_binary_dir, ConfigStore, ServiceType};
use std::process::Command;

/// Run a PostgreSQL tool with auto-connection
///
/// Finds the running PostgreSQL instance, resolves the versioned binary path,
/// injects connection arguments, and executes the tool with any additional args.
pub fn run_postgres(tool: &str, args: Vec<String>) -> Result<(), String> {
    // Load config to find running instances
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Find a PostgreSQL instance
    let instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::PostgreSQL)
        .ok_or_else(|| {
            "No PostgreSQL instance found.\n\
             Create one in the Burd app first."
                .to_string()
        })?;

    // Get the version - use instance version or find installed version
    let version = if instance.version.is_empty() {
        // Find any installed version for PostgreSQL
        config
            .binaries
            .get(&ServiceType::PostgreSQL)
            .and_then(|versions| versions.keys().next())
            .cloned()
            .ok_or_else(|| {
                "No PostgreSQL binary installed. Download one in the Burd app first.".to_string()
            })?
    } else {
        instance.version.clone()
    };

    // Get the versioned binary directory
    let bin_dir = get_versioned_binary_dir(ServiceType::PostgreSQL, &version)?;

    // Check if the tool exists in the bin directory
    let tool_path = bin_dir.join("bin").join(tool);
    if !tool_path.exists() {
        // Try without "bin" subdirectory (some packages have flat structure)
        let tool_path_flat = bin_dir.join(tool);
        if !tool_path_flat.exists() {
            return Err(format!(
                "Tool '{}' not found in PostgreSQL {}\nAvailable at: {}",
                tool,
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
            || a.starts_with("-p")
            || a.starts_with("--username")
            || a.starts_with("-U")
            || a.starts_with("--dbname")
            || a.starts_with("-d")
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

/// Build connection arguments for PostgreSQL
fn build_connection_args(instance: &crate::config::Instance) -> Vec<String> {
    let mut args = Vec::new();

    // Host and port
    args.push("--host=127.0.0.1".to_string());
    args.push(format!("--port={}", instance.port));

    // Add user
    let user = instance
        .config
        .get("user")
        .and_then(|v| v.as_str())
        .unwrap_or("postgres");
    args.push(format!("--username={}", user));

    // Note: PostgreSQL password is typically handled via PGPASSWORD env var or .pgpass file
    // We could set PGPASSWORD but that's less secure. For now, let user handle it.

    args
}

/// List available PostgreSQL tools
pub fn list_postgres_tools() -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let versions = match config.binaries.get(&ServiceType::PostgreSQL) {
        Some(v) if !v.is_empty() => v,
        _ => {
            println!("No PostgreSQL binary installed.");
            println!("Install one via the Burd app first.");
            return Ok(());
        }
    };

    let version = match versions.keys().next() {
        Some(v) => v,
        None => {
            println!("No PostgreSQL versions installed.");
            return Ok(());
        }
    };

    let bin_dir = get_versioned_binary_dir(ServiceType::PostgreSQL, version)?;
    let tools_dir = bin_dir.join("bin");

    if !tools_dir.exists() {
        println!("Binary directory not found: {}", tools_dir.display());
        return Ok(());
    }

    println!("Available PostgreSQL {} tools:", version);
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
