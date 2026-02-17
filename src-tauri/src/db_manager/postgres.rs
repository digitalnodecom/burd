//! PostgreSQL Database Manager
//!
//! Provides database operations using the psql CLI tools.

use super::{DatabaseInfo, DatabaseManager};
use std::path::Path;
use std::process::Command;

/// PostgreSQL database manager
pub struct PostgresManager {
    host: String,
    port: u16,
    user: String,
    password: Option<String>,
}

impl PostgresManager {
    /// Create a new PostgreSQL manager
    pub fn new(host: String, port: u16, user: String, password: Option<String>) -> Self {
        Self {
            host,
            port,
            user,
            password,
        }
    }

    /// Build base psql command with environment variables for password
    fn build_command(&self, cmd: &str) -> Command {
        let mut command = Command::new(cmd);

        command.arg("-h").arg(&self.host);
        command.arg("-p").arg(self.port.to_string());
        command.arg("-U").arg(&self.user);

        // Set PGPASSWORD environment variable if password is provided
        if let Some(ref password) = self.password {
            if !password.is_empty() {
                command.env("PGPASSWORD", password);
            }
        }

        command
    }

    /// Execute a SQL query and return the output
    fn execute_query(&self, query: &str) -> Result<String, String> {
        let mut cmd = self.build_command("psql");
        cmd.arg("-t"); // Tuples only (no headers)
        cmd.arg("-A"); // Unaligned output
        cmd.arg("-c").arg(query);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute psql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("PostgreSQL error: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute a SQL query on a specific database
    #[allow(dead_code)]
    fn execute_query_on_db(&self, database: &str, query: &str) -> Result<String, String> {
        let mut cmd = self.build_command("psql");
        cmd.arg("-d").arg(database);
        cmd.arg("-t");
        cmd.arg("-A");
        cmd.arg("-c").arg(query);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute psql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("PostgreSQL error: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl DatabaseManager for PostgresManager {
    fn list_databases(&self) -> Result<Vec<DatabaseInfo>, String> {
        let query = "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname";
        let output = self.execute_query(query)?;

        let databases: Vec<DatabaseInfo> = output
            .lines()
            .filter(|line| !line.is_empty())
            .filter(|name| {
                // Filter out system databases
                !matches!(*name, "postgres")
            })
            .map(|name| DatabaseInfo {
                name: name.to_string(),
                size: None,
                tables: None,
            })
            .collect();

        Ok(databases)
    }

    fn create_database(&self, name: &str) -> Result<(), String> {
        let sanitized = super::sanitize_db_name(name)?;

        // Check if exists first (CREATE DATABASE doesn't support IF NOT EXISTS in all versions)
        if self.database_exists(&sanitized)? {
            return Ok(()); // Already exists
        }

        let query = format!(
            "CREATE DATABASE \"{}\" ENCODING 'UTF8' LC_COLLATE 'en_US.UTF-8' LC_CTYPE 'en_US.UTF-8'",
            sanitized
        );

        // Try with locale settings first, fall back to simpler command
        let result = self.execute_query(&query);
        if result.is_err() {
            // Try without locale settings
            let simple_query = format!("CREATE DATABASE \"{}\"", sanitized);
            self.execute_query(&simple_query)?;
        }

        Ok(())
    }

    fn drop_database(&self, name: &str) -> Result<(), String> {
        let sanitized = super::sanitize_db_name(name)?;

        // Terminate existing connections first
        let terminate_query = format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = '{}'",
            sanitized
        );
        let _ = self.execute_query(&terminate_query); // Ignore errors

        let query = format!("DROP DATABASE IF EXISTS \"{}\"", sanitized);
        self.execute_query(&query)?;
        Ok(())
    }

    fn database_exists(&self, name: &str) -> Result<bool, String> {
        let sanitized = super::sanitize_db_name(name)?;
        let query = format!("SELECT 1 FROM pg_database WHERE datname = '{}'", sanitized);
        let output = self.execute_query(&query)?;
        Ok(!output.trim().is_empty())
    }

    fn import_sql(&self, database: &str, sql_path: &Path) -> Result<(), String> {
        let sanitized = super::sanitize_db_name(database)?;

        if !sql_path.exists() {
            return Err(format!("SQL file not found: {}", sql_path.display()));
        }

        let mut cmd = self.build_command("psql");
        cmd.arg("-d").arg(&sanitized);
        cmd.arg("-f").arg(sql_path);

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute psql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Import failed: {}", stderr));
        }

        Ok(())
    }

    fn export_sql(&self, database: &str, output_path: &Path) -> Result<(), String> {
        let sanitized = super::sanitize_db_name(database)?;

        let mut cmd = self.build_command("pg_dump");
        cmd.arg("-d").arg(&sanitized);
        cmd.arg("--no-owner");
        cmd.arg("--no-acl");

        let output = cmd
            .output()
            .map_err(|e| format!("Failed to execute pg_dump: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Export failed: {}", stderr));
        }

        std::fs::write(output_path, &output.stdout)
            .map_err(|e| format!("Failed to write SQL file: {}", e))?;

        Ok(())
    }

    fn get_shell_command(&self, database: Option<&str>) -> Vec<String> {
        let mut cmd = vec![
            "psql".to_string(),
            "-h".to_string(),
            self.host.clone(),
            "-p".to_string(),
            self.port.to_string(),
            "-U".to_string(),
            self.user.clone(),
        ];

        if let Some(db) = database {
            if let Ok(sanitized) = super::sanitize_db_name(db) {
                cmd.push("-d".to_string());
                cmd.push(sanitized);
            }
        }

        cmd
    }

    fn connection_info(&self) -> String {
        format!("PostgreSQL at {}:{}", self.host, self.port)
    }
}
