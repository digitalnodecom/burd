//! MariaDB/MySQL Database Manager
//!
//! Provides database operations using the mysql/mariadb CLI tools.

use super::{DatabaseInfo, DatabaseManager};
use std::path::Path;
use std::process::Command;

/// MariaDB database manager
pub struct MariaDbManager {
    host: String,
    port: u16,
    user: String,
    password: Option<String>,
    socket: Option<String>,
}

impl MariaDbManager {
    /// Create a new MariaDB manager
    pub fn new(
        host: String,
        port: u16,
        user: String,
        password: Option<String>,
        socket: Option<String>,
    ) -> Self {
        Self {
            host,
            port,
            user,
            password,
            socket,
        }
    }

    /// Build base mysql command arguments
    fn build_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        if let Some(ref socket) = self.socket {
            args.push(format!("--socket={}", socket));
        } else {
            args.push(format!("--host={}", self.host));
            args.push(format!("--port={}", self.port));
        }

        args.push(format!("--user={}", self.user));

        if let Some(ref password) = self.password {
            if !password.is_empty() {
                args.push(format!("--password={}", password));
            }
        }

        args
    }

    /// Find mysql binary (try mariadb first, then mysql)
    fn find_mysql_binary() -> String {
        // Check if mariadb command exists
        if Command::new("which")
            .arg("mariadb")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return "mariadb".to_string();
        }
        "mysql".to_string()
    }

    /// Find mysqldump binary
    fn find_mysqldump_binary() -> String {
        if Command::new("which")
            .arg("mariadb-dump")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return "mariadb-dump".to_string();
        }
        "mysqldump".to_string()
    }

    /// Execute a SQL query and return the output
    fn execute_query(&self, query: &str) -> Result<String, String> {
        let mysql = Self::find_mysql_binary();
        let mut args = self.build_args();
        args.push("-N".to_string()); // Skip column names
        args.push("-B".to_string()); // Batch mode (tab-separated)
        args.push("-e".to_string());
        args.push(query.to_string());

        let output = Command::new(&mysql)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute mysql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("MySQL error: {}", stderr));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl DatabaseManager for MariaDbManager {
    fn list_databases(&self) -> Result<Vec<DatabaseInfo>, String> {
        let output = self.execute_query("SHOW DATABASES")?;

        let databases: Vec<DatabaseInfo> = output
            .lines()
            .filter(|line| !line.is_empty())
            .filter(|name| {
                // Filter out system databases
                !matches!(
                    *name,
                    "information_schema" | "performance_schema" | "mysql" | "sys"
                )
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
        let query = format!(
            "CREATE DATABASE IF NOT EXISTS `{}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci",
            sanitized
        );
        self.execute_query(&query)?;
        Ok(())
    }

    fn drop_database(&self, name: &str) -> Result<(), String> {
        let sanitized = super::sanitize_db_name(name)?;
        let query = format!("DROP DATABASE IF EXISTS `{}`", sanitized);
        self.execute_query(&query)?;
        Ok(())
    }

    fn database_exists(&self, name: &str) -> Result<bool, String> {
        let sanitized = super::sanitize_db_name(name)?;
        let query = format!(
            "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = '{}'",
            sanitized
        );
        let output = self.execute_query(&query)?;
        Ok(!output.trim().is_empty())
    }

    fn import_sql(&self, database: &str, sql_path: &Path) -> Result<(), String> {
        let sanitized = super::sanitize_db_name(database)?;

        if !sql_path.exists() {
            return Err(format!("SQL file not found: {}", sql_path.display()));
        }

        let mysql = Self::find_mysql_binary();
        let mut args = self.build_args();
        args.push(sanitized);

        let sql_content = std::fs::read_to_string(sql_path)
            .map_err(|e| format!("Failed to read SQL file: {}", e))?;

        let mut child = Command::new(&mysql)
            .args(&args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn mysql: {}", e))?;

        use std::io::Write;
        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(sql_content.as_bytes())
                .map_err(|e| format!("Failed to write to mysql stdin: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait for mysql: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Import failed: {}", stderr));
        }

        Ok(())
    }

    fn export_sql(&self, database: &str, output_path: &Path) -> Result<(), String> {
        let sanitized = super::sanitize_db_name(database)?;

        let mysqldump = Self::find_mysqldump_binary();
        let mut args = self.build_args();
        args.push("--single-transaction".to_string());
        args.push("--routines".to_string());
        args.push("--triggers".to_string());
        args.push(sanitized);

        let output = Command::new(&mysqldump)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to execute mysqldump: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Export failed: {}", stderr));
        }

        std::fs::write(output_path, &output.stdout)
            .map_err(|e| format!("Failed to write SQL file: {}", e))?;

        Ok(())
    }

    fn get_shell_command(&self, database: Option<&str>) -> Vec<String> {
        let mysql = Self::find_mysql_binary();
        let mut cmd = vec![mysql];
        cmd.extend(self.build_args());

        if let Some(db) = database {
            if let Ok(sanitized) = super::sanitize_db_name(db) {
                cmd.push(sanitized);
            }
        }

        cmd
    }

    fn connection_info(&self) -> String {
        if let Some(ref socket) = self.socket {
            format!("MariaDB via socket {}", socket)
        } else {
            format!("MariaDB at {}:{}", self.host, self.port)
        }
    }
}
