//! Database Manager Module
//!
//! Provides database management operations for MariaDB and PostgreSQL.

pub mod mariadb;
pub mod postgres;

pub use mariadb::MariaDbManager;
pub use postgres::PostgresManager;

use crate::config::{Config, Instance, ServiceType};
use std::path::Path;

/// Database information
#[derive(Debug, Clone)]
pub struct DatabaseInfo {
    /// Database name
    pub name: String,
    /// Size in bytes (if available)
    pub size: Option<u64>,
    /// Number of tables (if available)
    pub tables: Option<u32>,
}

/// Trait for database management operations
pub trait DatabaseManager {
    /// List all databases
    fn list_databases(&self) -> Result<Vec<DatabaseInfo>, String>;

    /// Create a new database
    fn create_database(&self, name: &str) -> Result<(), String>;

    /// Drop a database
    fn drop_database(&self, name: &str) -> Result<(), String>;

    /// Check if a database exists
    fn database_exists(&self, name: &str) -> Result<bool, String>;

    /// Import SQL file into database
    fn import_sql(&self, database: &str, sql_path: &Path) -> Result<(), String>;

    /// Export database to SQL file
    fn export_sql(&self, database: &str, output_path: &Path) -> Result<(), String>;

    /// Get the shell command to open interactive database shell
    fn get_shell_command(&self, database: Option<&str>) -> Vec<String>;

    /// Get connection info for display
    fn connection_info(&self) -> String;
}

/// Database type enum
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DbType {
    MariaDB,
    PostgreSQL,
}

/// Find a database instance in Burd config
pub fn find_db_instance(config: &Config, db_type: Option<DbType>) -> Option<&Instance> {
    config.instances.iter().find(|i| match db_type {
        Some(DbType::MariaDB) => i.service_type == ServiceType::MariaDB,
        Some(DbType::PostgreSQL) => i.service_type == ServiceType::PostgreSQL,
        None => {
            i.service_type == ServiceType::MariaDB || i.service_type == ServiceType::PostgreSQL
        }
    })
}

/// Find all database instances in Burd config
pub fn find_all_db_instances(config: &Config) -> Vec<&Instance> {
    config
        .instances
        .iter()
        .filter(|i| {
            i.service_type == ServiceType::MariaDB || i.service_type == ServiceType::PostgreSQL
        })
        .collect()
}

/// Create a database manager for an instance
pub fn create_manager_for_instance(instance: &Instance) -> Result<Box<dyn DatabaseManager>, String> {
    match instance.service_type {
        ServiceType::MariaDB => {
            let socket = instance
                .config
                .get("socket")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let user = instance
                .config
                .get("user")
                .and_then(|v| v.as_str())
                .unwrap_or("root")
                .to_string();

            let password = instance
                .config
                .get("password")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            Ok(Box::new(MariaDbManager::new(
                "127.0.0.1".to_string(),
                instance.port,
                user,
                password,
                socket,
            )))
        }
        ServiceType::PostgreSQL => {
            let user = instance
                .config
                .get("user")
                .and_then(|v| v.as_str())
                .unwrap_or("postgres")
                .to_string();

            let password = instance
                .config
                .get("password")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            Ok(Box::new(PostgresManager::new(
                "127.0.0.1".to_string(),
                instance.port,
                user,
                password,
            )))
        }
        _ => Err(format!(
            "Instance '{}' is not a database service",
            instance.name
        )),
    }
}

/// Sanitize database name to prevent injection
pub fn sanitize_db_name(name: &str) -> Result<String, String> {
    // Only allow alphanumeric, underscore, and hyphen
    let sanitized: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    if sanitized.is_empty() {
        return Err("Database name cannot be empty".to_string());
    }

    if sanitized.len() > 64 {
        return Err("Database name too long (max 64 characters)".to_string());
    }

    // Must start with letter or underscore
    if !sanitized.chars().next().map(|c| c.is_alphabetic() || c == '_').unwrap_or(false) {
        return Err("Database name must start with a letter or underscore".to_string());
    }

    Ok(sanitized)
}
