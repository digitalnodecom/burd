//! PostgreSQL Database Manager
//!
//! Provides database operations using the psql CLI tools.

use super::{DatabaseInfo, DatabaseManager, DbUser, ExtensionInfo};
use std::path::{Path, PathBuf};
use std::process::Command;

/// PostgreSQL database manager
pub struct PostgresManager {
    host: String,
    port: u16,
    user: String,
    password: Option<String>,
    /// Directory holding Burd's bundled psql/pg_dump/… (the instance's
    /// `bin/`). The GUI process has no shell PATH, so we must invoke the
    /// bundled client by absolute path; falls back to PATH when unset.
    bin_dir: Option<PathBuf>,
}

impl PostgresManager {
    /// Create a new PostgreSQL manager
    pub fn new(host: String, port: u16, user: String, password: Option<String>) -> Self {
        Self {
            host,
            port,
            user,
            password,
            bin_dir: None,
        }
    }

    /// Point the manager at Burd's bundled client `bin/` directory.
    pub fn with_bin_dir(mut self, bin_dir: Option<PathBuf>) -> Self {
        self.bin_dir = bin_dir;
        self
    }

    /// Resolve a client tool to the bundled binary if present, else bare name
    /// (found via PATH — works when opened from a shell).
    fn resolve_bin(&self, name: &str) -> PathBuf {
        if let Some(dir) = &self.bin_dir {
            let p = dir.join(name);
            if p.exists() {
                return p;
            }
        }
        PathBuf::from(name)
    }

    /// Build base psql command with environment variables for password
    fn build_command(&self, cmd: &str) -> Command {
        let mut command = Command::new(self.resolve_bin(cmd));

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
    fn execute_query_on_db(&self, database: &str, query: &str) -> Result<String, String> {
        let mut cmd = self.build_command("psql");
        cmd.arg("-d").arg(database);
        cmd.arg("-t");
        cmd.arg("-A");
        cmd.arg("-F").arg("\u{1f}"); // unit-separator field delimiter
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

/// Validate a PostgreSQL extension name (interpolated into SQL). Allows the
/// lowercase-alnum, underscore and hyphen used by real extension names, and
/// nothing else — no quotes, semicolons or whitespace that could break out.
fn validate_extension_name(name: &str) -> Result<&str, String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(name)
    } else {
        Err(format!("Invalid extension name: '{}'", name))
    }
}

impl DatabaseManager for PostgresManager {
    fn list_databases(&self) -> Result<Vec<DatabaseInfo>, String> {
        // pg_database_size gives the on-disk size of each database in bytes.
        // execute_query runs psql with -t -A, so rows come back as
        // "datname|size" (default unaligned field separator is '|').
        let query = "SELECT datname, pg_database_size(datname) FROM pg_database \
             WHERE datistemplate = false ORDER BY datname";
        let output = self.execute_query(query)?;

        let databases: Vec<DatabaseInfo> = output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let mut parts = line.splitn(2, '|');
                let name = parts.next()?.trim().to_string();
                // Filter out system databases
                if name.is_empty() || name == "postgres" {
                    return None;
                }
                let size = parts.next().and_then(|s| s.trim().parse::<u64>().ok());
                Some(DatabaseInfo {
                    name,
                    size,
                    tables: None,
                })
            })
            .collect();

        Ok(databases)
    }

    fn list_users(&self) -> Result<Vec<DbUser>, String> {
        // Exclude the built-in pg_* roles; keep real users (incl. the postgres
        // superuser). execute_query runs psql -t -A (field separator '|'). The
        // role name goes last and we splitn(3) so a '|' inside a role name can't
        // shift the boolean columns.
        let query = "SELECT rolsuper, rolcanlogin, rolname FROM pg_roles \
             WHERE rolname NOT LIKE 'pg\\_%' ORDER BY rolname";
        let output = self.execute_query(query)?;

        let users = output
            .lines()
            .filter(|line| !line.is_empty())
            .filter_map(|line| {
                let mut parts = line.splitn(3, '|');
                let is_superuser = parts.next().map(|s| s.trim() == "t").unwrap_or(false);
                let can_login = parts.next().map(|s| s.trim() == "t").unwrap_or(false);
                let name = parts.next()?.trim().to_string();
                if name.is_empty() {
                    return None;
                }
                Some(DbUser {
                    name,
                    host: None,
                    is_superuser,
                    can_login,
                })
            })
            .collect();

        Ok(users)
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

    /// List extensions available in this database, marking which are installed.
    /// Covers both the bundled contrib extensions and the companion extensions
    /// Burd ships (pgvector, pg_partman).
    fn list_extensions(&self, database: &str) -> Result<Vec<ExtensionInfo>, String> {
        let query = "SELECT ae.name, ae.default_version, COALESCE(e.extversion, ''), \
             COALESCE(ae.comment, '') \
             FROM pg_available_extensions ae \
             LEFT JOIN pg_extension e ON e.extname = ae.name \
             ORDER BY (e.extversion IS NOT NULL) DESC, ae.name";
        let out = self.execute_query_on_db(database, query)?;
        let mut exts = Vec::new();
        for line in out.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let cols: Vec<&str> = line.split('\u{1f}').collect();
            if cols.len() < 4 {
                continue;
            }
            let installed_version = cols[2].trim();
            exts.push(ExtensionInfo {
                name: cols[0].trim().to_string(),
                default_version: cols[1].trim().to_string(),
                installed: !installed_version.is_empty(),
                installed_version: if installed_version.is_empty() {
                    None
                } else {
                    Some(installed_version.to_string())
                },
                comment: cols[3].trim().to_string(),
            });
        }
        Ok(exts)
    }

    fn enable_extension(&self, database: &str, extension: &str) -> Result<(), String> {
        let ext = validate_extension_name(extension)?;
        // ext is validated to a safe identifier; quote it to preserve case.
        let query = format!("CREATE EXTENSION IF NOT EXISTS \"{}\" CASCADE", ext);
        self.execute_query_on_db(database, &query).map(|_| ())
    }

    fn disable_extension(&self, database: &str, extension: &str) -> Result<(), String> {
        let ext = validate_extension_name(extension)?;
        let query = format!("DROP EXTENSION IF EXISTS \"{}\"", ext);
        self.execute_query_on_db(database, &query).map(|_| ())
    }
}
