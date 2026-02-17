//! Project analyzer types
//!
//! Data structures for representing analyzed project information.

use std::fmt;
use std::path::PathBuf;

/// Detected project type
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    /// Laravel framework project
    Laravel { version: Option<String> },
    /// Roots Bedrock WordPress setup
    Bedrock,
    /// Standard WordPress installation
    WordPress,
    /// Symfony framework project
    Symfony { version: Option<String> },
    /// Unknown or unsupported project type
    Unknown,
}

impl ProjectType {
    /// Get a display-friendly name for the project type
    pub fn display_name(&self) -> String {
        match self {
            ProjectType::Laravel { version: Some(v) } => format!("Laravel {}", v),
            ProjectType::Laravel { version: None } => "Laravel".to_string(),
            ProjectType::Bedrock => "Bedrock (WordPress)".to_string(),
            ProjectType::WordPress => "WordPress".to_string(),
            ProjectType::Symfony { version: Some(v) } => format!("Symfony {}", v),
            ProjectType::Symfony { version: None } => "Symfony".to_string(),
            ProjectType::Unknown => "Unknown".to_string(),
        }
    }

    /// Check if this project type uses .env files
    pub fn uses_env_file(&self) -> bool {
        matches!(
            self,
            ProjectType::Laravel { .. } | ProjectType::Bedrock | ProjectType::Symfony { .. }
        )
    }

    /// Check if this project type uses wp-config.php
    pub fn uses_wp_config(&self) -> bool {
        matches!(self, ProjectType::WordPress)
    }
}

impl fmt::Display for ProjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Database connection configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Connection type (mysql, pgsql, sqlite)
    pub connection: String,
    /// Database host
    pub host: String,
    /// Database port
    pub port: u16,
    /// Database name
    pub database: String,
    /// Database username
    pub username: String,
    /// Database password
    pub password: String,
}

impl DatabaseConfig {
    /// Create a new database config with default values
    pub fn new_mysql(database: String) -> Self {
        Self {
            connection: "mysql".to_string(),
            host: "127.0.0.1".to_string(),
            port: 3306,
            database,
            username: "root".to_string(),
            password: String::new(),
        }
    }

    /// Check if this is a MySQL/MariaDB connection
    pub fn is_mysql(&self) -> bool {
        self.connection == "mysql" || self.connection == "mariadb"
    }

    /// Check if this is a PostgreSQL connection
    pub fn is_postgres(&self) -> bool {
        self.connection == "pgsql"
            || self.connection == "postgres"
            || self.connection == "postgresql"
    }

    /// Check if this is a SQLite connection
    pub fn is_sqlite(&self) -> bool {
        self.connection == "sqlite"
    }
}

/// Cache/session configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Cache driver (redis, memcached, file, database, array)
    pub driver: String,
    /// Cache host (for Redis/Memcached)
    pub host: Option<String>,
    /// Cache port (for Redis/Memcached)
    pub port: Option<u16>,
}

impl CacheConfig {
    /// Check if this uses Redis
    pub fn is_redis(&self) -> bool {
        self.driver == "redis"
    }

    /// Check if this uses Memcached
    pub fn is_memcached(&self) -> bool {
        self.driver == "memcached"
    }
}

/// Mail configuration
#[derive(Debug, Clone)]
pub struct MailConfig {
    /// Mail driver/mailer (smtp, mailpit, sendmail, log)
    pub mailer: String,
    /// SMTP host
    pub host: String,
    /// SMTP port
    pub port: u16,
    /// SMTP username (optional)
    pub username: Option<String>,
    /// SMTP password (optional)
    pub password: Option<String>,
}

impl MailConfig {
    /// Check if this appears to be using Mailpit
    pub fn is_mailpit(&self) -> bool {
        self.mailer == "smtp"
            && (self.host == "127.0.0.1" || self.host == "localhost")
            && self.port == 1025
    }

    /// Check if this appears to be using Mailtrap
    pub fn is_mailtrap(&self) -> bool {
        self.host.contains("mailtrap")
    }
}

/// Search/Scout configuration (for Laravel)
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Search driver (meilisearch, algolia, database, collection)
    pub driver: String,
    /// Search host (for Meilisearch)
    pub host: Option<String>,
    /// Search API key
    pub key: Option<String>,
}

impl SearchConfig {
    /// Check if this uses Meilisearch
    pub fn is_meilisearch(&self) -> bool {
        self.driver == "meilisearch"
    }
}

/// Issue severity level
#[derive(Debug, Clone, PartialEq)]
pub enum IssueSeverity {
    /// Critical issue that will prevent the app from working
    Error,
    /// Issue that should be addressed but won't break the app
    Warning,
    /// Informational suggestion for improvement
    Info,
}

impl fmt::Display for IssueSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IssueSeverity::Error => write!(f, "Error"),
            IssueSeverity::Warning => write!(f, "Warning"),
            IssueSeverity::Info => write!(f, "Info"),
        }
    }
}

/// A detected issue or suggestion for the project
#[derive(Debug, Clone)]
pub struct ProjectIssue {
    /// Severity of the issue
    pub severity: IssueSeverity,
    /// Category (database, cache, mail, search, etc.)
    pub category: String,
    /// Description of the issue
    pub message: String,
    /// Suggested fix (if any)
    pub suggestion: Option<String>,
}

impl ProjectIssue {
    /// Create a new error issue
    pub fn error(category: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Error,
            category: category.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create a new warning issue
    pub fn warning(category: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Warning,
            category: category.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Create a new info issue
    pub fn info(category: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: IssueSeverity::Info,
            category: category.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Add a suggestion to this issue
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Complete analyzed project information
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    /// Detected project type
    pub project_type: ProjectType,
    /// Project name (usually directory name)
    pub name: String,
    /// Project root path
    pub path: PathBuf,
    /// Web document root (public/, web/, etc.)
    pub document_root: PathBuf,
    /// PHP version from composer.json require constraint
    pub php_version: Option<String>,
    /// Actual PHP version from the Burd instance serving this project
    pub instance_php_version: Option<String>,
    /// Database configuration
    pub database: Option<DatabaseConfig>,
    /// Cache configuration
    pub cache: Option<CacheConfig>,
    /// Mail configuration
    pub mail: Option<MailConfig>,
    /// Search configuration
    pub search: Option<SearchConfig>,
    /// Path to .env file (if exists)
    pub env_file: Option<PathBuf>,
    /// Detected issues and suggestions
    pub issues: Vec<ProjectIssue>,
}

impl ProjectInfo {
    /// Create a new ProjectInfo for an unknown project
    pub fn unknown(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        Self {
            project_type: ProjectType::Unknown,
            name,
            document_root: path.clone(),
            path,
            php_version: None,
            instance_php_version: None,
            database: None,
            cache: None,
            mail: None,
            search: None,
            env_file: None,
            issues: Vec::new(),
        }
    }

    /// Check if there are any error-level issues
    pub fn has_errors(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Error)
    }

    /// Check if there are any warning-level issues
    pub fn has_warnings(&self) -> bool {
        self.issues
            .iter()
            .any(|i| i.severity == IssueSeverity::Warning)
    }

    /// Get issues filtered by category
    pub fn issues_for_category(&self, category: &str) -> Vec<&ProjectIssue> {
        self.issues
            .iter()
            .filter(|i| i.category == category)
            .collect()
    }

    /// Add an issue to this project
    pub fn add_issue(&mut self, issue: ProjectIssue) {
        self.issues.push(issue);
    }
}

/// Parsed composer.json information
#[derive(Debug, Clone, Default)]
pub struct ComposerInfo {
    /// Project name from composer.json
    pub name: Option<String>,
    /// Required dependencies
    pub require: std::collections::HashMap<String, String>,
    /// Dev dependencies
    pub require_dev: std::collections::HashMap<String, String>,
}

impl ComposerInfo {
    /// Check if a dependency exists (in require or require-dev)
    pub fn has_dependency(&self, package: &str) -> bool {
        self.require.contains_key(package) || self.require_dev.contains_key(package)
    }

    /// Get the version constraint for a dependency
    pub fn get_version(&self, package: &str) -> Option<&String> {
        self.require
            .get(package)
            .or_else(|| self.require_dev.get(package))
    }

    /// Extract a simple version number from a constraint
    /// e.g., "^11.0" -> "11", "~10.0" -> "10"
    pub fn get_major_version(&self, package: &str) -> Option<String> {
        self.get_version(package).and_then(|v| {
            // Remove prefixes like ^, ~, >=, etc.
            let cleaned = v
                .trim_start_matches('^')
                .trim_start_matches('~')
                .trim_start_matches(">=")
                .trim_start_matches('>')
                .trim_start_matches("<=")
                .trim_start_matches('<')
                .trim_start_matches('=');

            // Extract major version
            cleaned.split('.').next().map(|s| s.to_string())
        })
    }
}
