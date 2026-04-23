//! Database CLI commands
//!
//! Commands for managing databases from the command line.

use crate::config::{ConfigStore, Instance, ServiceType};
use crate::db_manager::{create_manager_for_instance, find_all_db_instances, sanitize_db_name, DbType};
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// List all databases. Probes each engine in parallel with a per-engine
/// timeout so one down instance can't stall the others.
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

    // Launch one thread per instance; each reports into an mpsc channel so the
    // slowest doesn't block output of the fastest.
    let (tx, rx) = mpsc::channel::<(String, String, Result<Vec<String>, String>)>();
    let mut threads = Vec::with_capacity(db_instances.len());

    for instance in &db_instances {
        let tx = tx.clone();
        let name = instance.name.clone();
        let instance_clone = (*instance).clone();
        threads.push(thread::spawn(move || {
            let (conn_info, result) = match create_manager_for_instance(&instance_clone) {
                Ok(m) => {
                    let info = m.connection_info();
                    let res = m
                        .list_databases()
                        .map(|v| v.into_iter().map(|d| d.name).collect::<Vec<_>>());
                    (info, res)
                }
                Err(e) => (String::new(), Err(e)),
            };
            let _ = tx.send((name, conn_info, result));
        }));
    }
    drop(tx);

    // Per-engine probe budget is 500ms — mirrors doctor's liveness check.
    // Any engine that hasn't responded by the overall deadline is reported
    // as unreachable rather than stalling the whole listing.
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    let mut collected: Vec<(String, String, Result<Vec<String>, String>)> = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(entry) => collected.push(entry),
            Err(_) => break,
        }
        if collected.len() == db_instances.len() {
            break;
        }
    }

    // Preserve config order in output.
    let mut printed = vec![false; db_instances.len()];
    for (idx, instance) in db_instances.iter().enumerate() {
        if let Some(entry) = collected.iter().find(|(n, _, _)| n == &instance.name) {
            let (_, conn_info, result) = entry;
            println!();
            println!("{} ({})", instance.name, conn_info);
            println!("{}", "-".repeat(40));
            match result {
                Ok(dbs) if dbs.is_empty() => println!("  (no databases)"),
                Ok(dbs) => {
                    for db in dbs {
                        println!("  {}", db);
                    }
                }
                Err(e) => println!("  Error: {}", e),
            }
            printed[idx] = true;
        }
    }

    // Anything still running hit the deadline — report but don't block.
    for (idx, instance) in db_instances.iter().enumerate() {
        if !printed[idx] {
            println!();
            println!("{} (unreachable)", instance.name);
            println!("{}", "-".repeat(40));
            println!("  Timed out after 500ms — is the instance running?");
        }
    }

    println!();
    Ok(())
}

/// Select an instance from the available db instances using the user-provided
/// filters. Precedence: `instance_name` (exact match) > `engine` filter > auto.
/// When `announce_auto` is true and we picked automatically from >1 candidate
/// engine, we print a line telling the user which engine we used — per
/// team-lead's rule: "if unambiguous, pick + print which one used."
///
/// Errors when the instance name doesn't match, when the engine has no match,
/// or when no filters were provided but multiple engines exist.
fn select_db_instance<'a>(
    instances: &'a [&'a Instance],
    engine: Option<DbType>,
    instance_name: Option<&str>,
    announce_auto: bool,
) -> Result<&'a Instance, String> {
    if let Some(name) = instance_name {
        return instances
            .iter()
            .copied()
            .find(|i| i.name == name)
            .ok_or_else(|| {
                format!(
                    "No database instance named '{}'. Available: {}",
                    name,
                    instances
                        .iter()
                        .map(|i| i.name.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            });
    }

    let matches_engine = |inst: &Instance, e: DbType| match e {
        DbType::MariaDB => inst.service_type == ServiceType::MariaDB,
        DbType::PostgreSQL => inst.service_type == ServiceType::PostgreSQL,
    };

    if let Some(e) = engine {
        return instances
            .iter()
            .copied()
            .find(|i| matches_engine(i, e))
            .ok_or_else(|| {
                format!(
                    "No {:?} instance configured. Available: {}",
                    e,
                    instances
                        .iter()
                        .map(|i| format!("{} ({:?})", i.name, i.service_type))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            });
    }

    let has_maria = instances
        .iter()
        .any(|i| i.service_type == ServiceType::MariaDB);
    let has_pg = instances
        .iter()
        .any(|i| i.service_type == ServiceType::PostgreSQL);

    if has_maria && has_pg {
        return Err(
            "Multiple database engines available. Pass --engine mariadb|postgres or --instance <name>.".to_string(),
        );
    }

    let picked = instances
        .first()
        .copied()
        .ok_or_else(|| "No database instances configured.".to_string())?;

    if announce_auto && instances.len() > 1 {
        eprintln!(
            "Using {} instance '{}' (auto-selected)",
            match picked.service_type {
                ServiceType::MariaDB => "MariaDB",
                ServiceType::PostgreSQL => "PostgreSQL",
                _ => "database",
            },
            picked.name
        );
    }

    Ok(picked)
}

/// Locate the instance that already contains `sanitized`. If multiple match
/// and `engine`/`instance_name` narrow it down, honor the filter.
fn find_instance_with_database<'a>(
    instances: &'a [&'a Instance],
    sanitized: &str,
    engine: Option<DbType>,
    instance_name: Option<&str>,
) -> Result<Option<&'a Instance>, String> {
    let matches_engine = |inst: &Instance, e: DbType| match e {
        DbType::MariaDB => inst.service_type == ServiceType::MariaDB,
        DbType::PostgreSQL => inst.service_type == ServiceType::PostgreSQL,
    };

    for inst in instances {
        if let Some(name) = instance_name {
            if inst.name != name {
                continue;
            }
        }
        if let Some(e) = engine {
            if !matches_engine(inst, e) {
                continue;
            }
        }
        let manager = create_manager_for_instance(inst)?;
        if manager.database_exists(sanitized)? {
            return Ok(Some(*inst));
        }
    }

    Ok(None)
}

/// Create a new database. When more than one engine is available,
/// `engine` or `instance_name` must be provided — otherwise we'd silently
/// pick one. Auto-selection prints an announcement.
pub fn run_db_create(
    name: &str,
    engine: Option<DbType>,
    instance_name: Option<&str>,
) -> Result<(), String> {
    let sanitized = sanitize_db_name(name)?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.\n\
             Create a MariaDB or PostgreSQL instance in the Burd app first."
            .to_string());
    }

    let instance = select_db_instance(&db_instances, engine, instance_name, true)?;
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
pub fn run_db_drop(
    name: &str,
    force: bool,
    engine: Option<DbType>,
    instance_name: Option<&str>,
) -> Result<(), String> {
    let sanitized = sanitize_db_name(name)?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.".to_string());
    }

    let instance = match find_instance_with_database(
        &db_instances,
        &sanitized,
        engine,
        instance_name,
    )? {
        Some(i) => i,
        None => return Err(format!("Database '{}' not found.", sanitized)),
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
pub fn run_db_import(
    name: &str,
    sql_file: &str,
    engine: Option<DbType>,
    instance_name: Option<&str>,
) -> Result<(), String> {
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

    let instance = match find_instance_with_database(
        &db_instances,
        &sanitized,
        engine,
        instance_name,
    )? {
        Some(i) => i,
        None => {
            // Database doesn't exist - offer to create it
            print!("Database '{}' doesn't exist. Create it? [Y/n] ", sanitized);
            io::stdout().flush().unwrap();

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .map_err(|e| format!("Failed to read input: {}", e))?;

            if input.trim().eq_ignore_ascii_case("n") {
                println!("Aborted.");
                return Ok(());
            }

            let target = select_db_instance(&db_instances, engine, instance_name, true)?;
            let manager = create_manager_for_instance(target)?;
            println!("Creating database '{}'...", sanitized);
            manager.create_database(&sanitized)?;
            target
        }
    };

    let manager = create_manager_for_instance(instance)?;

    println!("Importing {} into '{}'...", sql_file, sanitized);
    manager.import_sql(&sanitized, &sql_path)?;
    println!("Import completed successfully.");

    Ok(())
}

/// Export database to SQL file
pub fn run_db_export(
    name: &str,
    output_file: Option<&str>,
    engine: Option<DbType>,
    instance_name: Option<&str>,
) -> Result<(), String> {
    let sanitized = sanitize_db_name(name)?;

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.".to_string());
    }

    let instance = match find_instance_with_database(
        &db_instances,
        &sanitized,
        engine,
        instance_name,
    )? {
        Some(i) => i,
        None => return Err(format!("Database '{}' not found.", sanitized)),
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
pub fn run_db_shell(
    name: Option<&str>,
    engine: Option<DbType>,
    instance_name: Option<&str>,
) -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let db_instances = find_all_db_instances(&config);

    if db_instances.is_empty() {
        return Err("No database instances configured in Burd.".to_string());
    }

    let instance = if let Some(db_name) = name {
        let sanitized = sanitize_db_name(db_name)?;
        match find_instance_with_database(&db_instances, &sanitized, engine, instance_name)? {
            Some(i) => i,
            None => return Err(format!("Database '{}' not found.", sanitized)),
        }
    } else {
        select_db_instance(&db_instances, engine, instance_name, true)?
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
