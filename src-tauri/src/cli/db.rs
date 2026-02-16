//! Database CLI commands
//!
//! Commands for managing databases from the command line.

use crate::config::ConfigStore;
use crate::db_manager::{create_manager_for_instance, find_all_db_instances, sanitize_db_name};
use std::io::{self, Write};
use std::path::PathBuf;

/// List all databases
pub fn run_db_list() -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        println!("No database instances configured in Burd.");
        println!();
        println!("Create a MariaDB or PostgreSQL instance in the Burd app first.");
        return Ok(());
    }

    for instance in db_instances {
        let manager = create_manager_for_instance(instance)?;

        println!();
        println!("{} ({})", instance.name, manager.connection_info());
        println!("{}", "-".repeat(40));

        match manager.list_databases() {
            Ok(databases) => {
                if databases.is_empty() {
                    println!("  (no databases)");
                } else {
                    for db in databases {
                        println!("  {}", db.name);
                    }
                }
            }
            Err(e) => {
                println!("  Error: {}", e);
            }
        }
    }

    println!();
    Ok(())
}

/// Create a new database
pub fn run_db_create(name: &str) -> Result<(), String> {
    let sanitized = sanitize_db_name(name)?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err(
            "No database instances configured in Burd.\n\
             Create a MariaDB or PostgreSQL instance in the Burd app first."
                .to_string(),
        );
    }

    // Use the first database instance (typically MariaDB)
    let instance = db_instances[0];
    let manager = create_manager_for_instance(instance)?;

    // Check if database already exists
    if manager.database_exists(&sanitized)? {
        println!("Database '{}' already exists.", sanitized);
        return Ok(());
    }

    println!("Creating database '{}'...", sanitized);
    manager.create_database(&sanitized)?;
    println!("Database '{}' created successfully.", sanitized);

    Ok(())
}

/// Drop a database
pub fn run_db_drop(name: &str, force: bool) -> Result<(), String> {
    let sanitized = sanitize_db_name(name)?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.".to_string());
    }

    // Find which instance has this database
    let mut found_instance = None;
    for instance in &db_instances {
        let manager = create_manager_for_instance(instance)?;
        if manager.database_exists(&sanitized)? {
            found_instance = Some(instance);
            break;
        }
    }

    let instance = match found_instance {
        Some(i) => i,
        None => {
            return Err(format!("Database '{}' not found.", sanitized));
        }
    };

    let manager = create_manager_for_instance(instance)?;

    // Confirm deletion unless --force is passed
    if !force {
        print!(
            "Are you sure you want to drop database '{}'? This cannot be undone. [y/N] ",
            sanitized
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {}", e))?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!("Dropping database '{}'...", sanitized);
    manager.drop_database(&sanitized)?;
    println!("Database '{}' dropped successfully.", sanitized);

    Ok(())
}

/// Import SQL file into database
pub fn run_db_import(name: &str, sql_file: &str) -> Result<(), String> {
    let sanitized = sanitize_db_name(name)?;
    let sql_path = PathBuf::from(sql_file);

    if !sql_path.exists() {
        return Err(format!("SQL file not found: {}", sql_file));
    }

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.".to_string());
    }

    // Find which instance has this database (or use first one)
    let mut target_instance = None;
    for instance in &db_instances {
        let manager = create_manager_for_instance(instance)?;
        if manager.database_exists(&sanitized)? {
            target_instance = Some(instance);
            break;
        }
    }

    let instance = match target_instance {
        Some(i) => i,
        None => {
            // Database doesn't exist - offer to create it
            print!(
                "Database '{}' doesn't exist. Create it? [Y/n] ",
                sanitized
            );
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| format!("Failed to read input: {}", e))?;

            if input.trim().eq_ignore_ascii_case("n") {
                println!("Aborted.");
                return Ok(());
            }

            let instance = db_instances[0];
            let manager = create_manager_for_instance(instance)?;
            println!("Creating database '{}'...", sanitized);
            manager.create_database(&sanitized)?;
            instance
        }
    };

    let manager = create_manager_for_instance(instance)?;

    println!("Importing {} into '{}'...", sql_file, sanitized);
    manager.import_sql(&sanitized, &sql_path)?;
    println!("Import completed successfully.");

    Ok(())
}

/// Export database to SQL file
pub fn run_db_export(name: &str, output_file: Option<&str>) -> Result<(), String> {
    let sanitized = sanitize_db_name(name)?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.".to_string());
    }

    // Find which instance has this database
    let mut found_instance = None;
    for instance in &db_instances {
        let manager = create_manager_for_instance(instance)?;
        if manager.database_exists(&sanitized)? {
            found_instance = Some(instance);
            break;
        }
    }

    let instance = match found_instance {
        Some(i) => i,
        None => {
            return Err(format!("Database '{}' not found.", sanitized));
        }
    };

    let manager = create_manager_for_instance(instance)?;

    // Determine output path
    let output_path = match output_file {
        Some(f) => PathBuf::from(f),
        None => PathBuf::from(format!("{}.sql", sanitized)),
    };

    // Check if file exists
    if output_path.exists() {
        print!(
            "File '{}' already exists. Overwrite? [y/N] ",
            output_path.display()
        );
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .map_err(|e| format!("Failed to read input: {}", e))?;

        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    println!("Exporting '{}' to {}...", sanitized, output_path.display());
    manager.export_sql(&sanitized, &output_path)?;
    println!("Export completed: {}", output_path.display());

    Ok(())
}

/// Open interactive database shell
pub fn run_db_shell(name: Option<&str>) -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.".to_string());
    }

    // If database name is provided, find the instance that has it
    let instance = if let Some(db_name) = name {
        let sanitized = sanitize_db_name(db_name)?;
        let mut found = None;

        for inst in &db_instances {
            let manager = create_manager_for_instance(inst)?;
            if manager.database_exists(&sanitized)? {
                found = Some(*inst);
                break;
            }
        }

        match found {
            Some(i) => i,
            None => {
                return Err(format!("Database '{}' not found.", sanitized));
            }
        }
    } else {
        // Use first database instance
        db_instances[0]
    };

    let manager = create_manager_for_instance(instance)?;
    let shell_cmd = manager.get_shell_command(name);

    if shell_cmd.is_empty() {
        return Err("Failed to build shell command".to_string());
    }

    println!("Connecting to {}...", manager.connection_info());
    println!("Type 'exit' or Ctrl+D to quit.");
    println!();

    // Execute the shell command
    let status = std::process::Command::new(&shell_cmd[0])
        .args(&shell_cmd[1..])
        .status()
        .map_err(|e| format!("Failed to start database shell: {}", e))?;

    if !status.success() {
        return Err("Database shell exited with error".to_string());
    }

    Ok(())
}
