//! Configuration file parsers
//!
//! Parsers for various config file formats used by PHP projects.

use super::types::{
    CacheConfig, ComposerInfo, DatabaseConfig, MailConfig, ProjectType, SearchConfig,
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Parse a .env file into a HashMap
///
/// Handles:
/// - KEY=value pairs
/// - Quoted values (single and double)
/// - Comments (lines starting with #)
/// - Empty lines
pub fn parse_env_file(path: &Path) -> Option<HashMap<String, String>> {
    let env_path = if path.is_file() {
        path.to_path_buf()
    } else {
        path.join(".env")
    };

    let content = fs::read_to_string(&env_path).ok()?;
    Some(parse_env_content(&content))
}

/// Parse .env content string into HashMap
fn parse_env_content(content: &str) -> HashMap<String, String> {
    let mut env = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Find the first = sign
        if let Some(eq_pos) = line.find('=') {
            let key = line[..eq_pos].trim().to_string();
            let mut value = line[eq_pos + 1..].trim().to_string();

            // Remove surrounding quotes
            if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                value = value[1..value.len() - 1].to_string();
            }

            // Handle inline comments (only if not in quotes)
            if !value.contains('"') && !value.contains('\'') {
                if let Some(comment_pos) = value.find(" #") {
                    value = value[..comment_pos].trim().to_string();
                }
            }

            env.insert(key, value);
        }
    }

    env
}

/// Parse composer.json file
pub fn parse_composer_json(path: &Path) -> Option<ComposerInfo> {
    let composer_path = if path.is_file()
        && path
            .file_name()
            .map(|n| n == "composer.json")
            .unwrap_or(false)
    {
        path.to_path_buf()
    } else {
        path.join("composer.json")
    };

    let content = fs::read_to_string(&composer_path).ok()?;
    parse_composer_content(&content)
}

/// Parse composer.json content
fn parse_composer_content(content: &str) -> Option<ComposerInfo> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;

    let mut info = ComposerInfo {
        name: json
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        ..Default::default()
    };

    // Parse require
    if let Some(require) = json.get("require").and_then(|v| v.as_object()) {
        for (key, value) in require {
            if let Some(version) = value.as_str() {
                info.require.insert(key.clone(), version.to_string());
            }
        }
    }

    // Parse require-dev
    if let Some(require_dev) = json.get("require-dev").and_then(|v| v.as_object()) {
        for (key, value) in require_dev {
            if let Some(version) = value.as_str() {
                info.require_dev.insert(key.clone(), version.to_string());
            }
        }
    }

    Some(info)
}

/// Parse wp-config.php for database configuration
///
/// Extracts database settings from WordPress define() calls.
pub fn parse_wp_config(path: &Path) -> Option<DatabaseConfig> {
    let config_path = if path.is_file() {
        path.to_path_buf()
    } else {
        path.join("wp-config.php")
    };

    let content = fs::read_to_string(&config_path).ok()?;
    parse_wp_config_content(&content)
}

/// Parse wp-config.php content for database settings
fn parse_wp_config_content(content: &str) -> Option<DatabaseConfig> {
    let mut db_name = None;
    let mut db_user = None;
    let mut db_password = None;
    let mut db_host = None;

    // Match define('CONSTANT', 'value') patterns
    let define_pattern =
        regex::Regex::new(r#"define\s*\(\s*['"](\w+)['"]\s*,\s*['"]([^'"]*)['"]\s*\)"#).ok()?;

    for cap in define_pattern.captures_iter(content) {
        let name = cap.get(1).map(|m| m.as_str())?;
        let value = cap.get(2).map(|m| m.as_str().to_string())?;

        match name {
            "DB_NAME" => db_name = Some(value),
            "DB_USER" => db_user = Some(value),
            "DB_PASSWORD" => db_password = Some(value),
            "DB_HOST" => db_host = Some(value),
            _ => {}
        }
    }

    // Need at least database name to create config
    let database = db_name?;

    // Parse host:port from DB_HOST
    let host_str = db_host.unwrap_or_else(|| "localhost".to_string());
    let (host, port) = if host_str.contains(':') {
        let parts: Vec<&str> = host_str.splitn(2, ':').collect();
        (
            parts[0].to_string(),
            parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(3306),
        )
    } else {
        (host_str, 3306)
    };

    Some(DatabaseConfig {
        connection: "mysql".to_string(),
        host,
        port,
        database,
        username: db_user.unwrap_or_else(|| "root".to_string()),
        password: db_password.unwrap_or_default(),
    })
}

/// Extract database configuration from parsed environment variables
pub fn extract_database_config(
    project_type: &ProjectType,
    env: &HashMap<String, String>,
) -> Option<DatabaseConfig> {
    match project_type {
        ProjectType::Laravel { .. } | ProjectType::Symfony { .. } => {
            extract_laravel_database_config(env)
        }
        ProjectType::Bedrock => extract_bedrock_database_config(env),
        _ => None,
    }
}

/// Extract database config from Laravel-style .env
fn extract_laravel_database_config(env: &HashMap<String, String>) -> Option<DatabaseConfig> {
    // Get connection type
    let connection = env
        .get("DB_CONNECTION")
        .cloned()
        .unwrap_or_else(|| "mysql".to_string());

    // SQLite doesn't need host/port
    if connection == "sqlite" {
        let database = env
            .get("DB_DATABASE")
            .cloned()
            .unwrap_or_else(|| "database.sqlite".to_string());
        return Some(DatabaseConfig {
            connection,
            host: String::new(),
            port: 0,
            database,
            username: String::new(),
            password: String::new(),
        });
    }

    // Get host and port
    // Handle cases where DB_HOST might contain port (e.g., "127.0.0.1:3306")
    let host_raw = env
        .get("DB_HOST")
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let (host, host_port) = if host_raw.contains(':') {
        let parts: Vec<&str> = host_raw.splitn(2, ':').collect();
        (
            parts[0].to_string(),
            parts.get(1).and_then(|p| p.parse().ok()),
        )
    } else {
        (host_raw, None)
    };

    // Prefer explicit DB_PORT over port embedded in host
    let port = env
        .get("DB_PORT")
        .and_then(|p| p.parse().ok())
        .or(host_port)
        .unwrap_or_else(|| if connection == "pgsql" { 5432 } else { 3306 });

    let database = env.get("DB_DATABASE").cloned()?;
    let username = env
        .get("DB_USERNAME")
        .cloned()
        .unwrap_or_else(|| "root".to_string());
    let password = env.get("DB_PASSWORD").cloned().unwrap_or_default();

    Some(DatabaseConfig {
        connection,
        host,
        port,
        database,
        username,
        password,
    })
}

/// Extract database config from Bedrock-style .env
fn extract_bedrock_database_config(env: &HashMap<String, String>) -> Option<DatabaseConfig> {
    // Bedrock can use DATABASE_URL or individual vars
    if let Some(url) = env.get("DATABASE_URL") {
        return parse_database_url(url);
    }

    // Bedrock uses different variable names:
    // - DB_NAME instead of DB_DATABASE
    // - DB_USER instead of DB_USERNAME
    // - May not have DB_HOST/DB_PORT (defaults to localhost:3306)

    let database = env
        .get("DB_NAME")
        .or_else(|| env.get("DB_DATABASE"))
        .cloned()?;

    // Get host and port (handle embedded port in host)
    let host_raw = env
        .get("DB_HOST")
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let (host, host_port) = if host_raw.contains(':') {
        let parts: Vec<&str> = host_raw.splitn(2, ':').collect();
        (
            parts[0].to_string(),
            parts.get(1).and_then(|p| p.parse().ok()),
        )
    } else {
        (host_raw, None)
    };

    let port = env
        .get("DB_PORT")
        .and_then(|p| p.parse().ok())
        .or(host_port)
        .unwrap_or(3306);

    let username = env
        .get("DB_USER")
        .or_else(|| env.get("DB_USERNAME"))
        .cloned()
        .unwrap_or_else(|| "root".to_string());

    let password = env.get("DB_PASSWORD").cloned().unwrap_or_default();

    Some(DatabaseConfig {
        connection: "mysql".to_string(),
        host,
        port,
        database,
        username,
        password,
    })
}

/// Parse a DATABASE_URL into DatabaseConfig
fn parse_database_url(url: &str) -> Option<DatabaseConfig> {
    // Format: mysql://user:pass@host:port/database
    let url = url::Url::parse(url).ok()?;

    let connection = url.scheme().to_string();
    let host = url.host_str().unwrap_or("127.0.0.1").to_string();
    let port = url
        .port()
        .unwrap_or(if connection == "pgsql" { 5432 } else { 3306 });
    let database = url.path().trim_start_matches('/').to_string();
    let username = url.username().to_string();
    let password = url.password().unwrap_or("").to_string();

    Some(DatabaseConfig {
        connection,
        host,
        port,
        database,
        username,
        password,
    })
}

/// Extract cache configuration from environment
pub fn extract_cache_config(
    project_type: &ProjectType,
    env: &HashMap<String, String>,
) -> Option<CacheConfig> {
    match project_type {
        ProjectType::Laravel { .. } => extract_laravel_cache_config(env),
        _ => None,
    }
}

/// Extract Laravel cache configuration
fn extract_laravel_cache_config(env: &HashMap<String, String>) -> Option<CacheConfig> {
    let driver = env
        .get("CACHE_DRIVER")
        .or_else(|| env.get("CACHE_STORE"))
        .cloned()
        .unwrap_or_else(|| "file".to_string());

    let (host, port) = if driver == "redis" {
        let host = env.get("REDIS_HOST").cloned();
        let port = env.get("REDIS_PORT").and_then(|p| p.parse().ok());
        (host, port)
    } else if driver == "memcached" {
        let host = env.get("MEMCACHED_HOST").cloned();
        let port = env.get("MEMCACHED_PORT").and_then(|p| p.parse().ok());
        (host, port)
    } else {
        (None, None)
    };

    Some(CacheConfig { driver, host, port })
}

/// Extract mail configuration from environment
pub fn extract_mail_config(
    project_type: &ProjectType,
    env: &HashMap<String, String>,
) -> Option<MailConfig> {
    match project_type {
        ProjectType::Laravel { .. } => extract_laravel_mail_config(env),
        ProjectType::Bedrock | ProjectType::WordPress => None, // WordPress uses plugins
        _ => None,
    }
}

/// Extract Laravel mail configuration
fn extract_laravel_mail_config(env: &HashMap<String, String>) -> Option<MailConfig> {
    let mailer = env
        .get("MAIL_MAILER")
        .or_else(|| env.get("MAIL_DRIVER"))
        .cloned()
        .unwrap_or_else(|| "smtp".to_string());

    // Only extract host/port for SMTP
    if mailer != "smtp" {
        return Some(MailConfig {
            mailer,
            host: String::new(),
            port: 0,
            username: None,
            password: None,
        });
    }

    let host = env
        .get("MAIL_HOST")
        .cloned()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = env
        .get("MAIL_PORT")
        .and_then(|p| p.parse().ok())
        .unwrap_or(1025);
    let username = env.get("MAIL_USERNAME").cloned();
    let password = env.get("MAIL_PASSWORD").cloned();

    Some(MailConfig {
        mailer,
        host,
        port,
        username,
        password,
    })
}

/// Extract search/scout configuration from environment
pub fn extract_search_config(
    project_type: &ProjectType,
    env: &HashMap<String, String>,
) -> Option<SearchConfig> {
    match project_type {
        ProjectType::Laravel { .. } => extract_laravel_search_config(env),
        _ => None,
    }
}

/// Extract Laravel Scout search configuration
fn extract_laravel_search_config(env: &HashMap<String, String>) -> Option<SearchConfig> {
    let driver = env.get("SCOUT_DRIVER").cloned()?;

    let (host, key) = if driver == "meilisearch" {
        (
            env.get("MEILISEARCH_HOST").cloned(),
            env.get("MEILISEARCH_KEY").cloned(),
        )
    } else if driver == "algolia" {
        (None, env.get("ALGOLIA_APP_ID").cloned())
    } else {
        (None, None)
    };

    Some(SearchConfig { driver, host, key })
}

/// Extract PHP version requirement from composer.json
pub fn extract_php_version(composer: &ComposerInfo) -> Option<String> {
    composer.get_version("php").map(|v| {
        // Clean up version constraint
        v.trim_start_matches('^')
            .trim_start_matches('~')
            .trim_start_matches(">=")
            .split('|')
            .next()
            .unwrap_or(v)
            .trim()
            .to_string()
    })
}

/// Update a value in an .env file
///
/// Creates the key if it doesn't exist, updates if it does.
pub fn update_env_value(path: &Path, key: &str, value: &str) -> Result<(), String> {
    let env_path = if path.is_file() {
        path.to_path_buf()
    } else {
        path.join(".env")
    };

    let content =
        fs::read_to_string(&env_path).map_err(|e| format!("Failed to read .env file: {}", e))?;

    let new_content = update_env_content(&content, key, value);

    fs::write(&env_path, new_content).map_err(|e| format!("Failed to write .env file: {}", e))?;

    Ok(())
}

/// Update a key-value pair in .env content
fn update_env_content(content: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let key_prefix = format!("{}=", key);
    let new_line = format!("{}={}", key, value);

    // Find and update existing key
    let mut found = false;
    for line in &mut lines {
        let trimmed = line.trim();
        if trimmed.starts_with(&key_prefix) && !trimmed.starts_with('#') {
            *line = new_line.clone();
            found = true;
            break;
        }
    }

    // Add new key if not found
    if !found {
        lines.push(new_line);
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_env_content() {
        let content = r#"
APP_NAME=MyApp
APP_ENV=local
DB_HOST=127.0.0.1
DB_PORT=3306
DB_DATABASE=myapp
# This is a comment
QUOTED="hello world"
SINGLE='single quotes'
"#;

        let env = parse_env_content(content);

        assert_eq!(env.get("APP_NAME"), Some(&"MyApp".to_string()));
        assert_eq!(env.get("DB_HOST"), Some(&"127.0.0.1".to_string()));
        assert_eq!(env.get("DB_PORT"), Some(&"3306".to_string()));
        assert_eq!(env.get("QUOTED"), Some(&"hello world".to_string()));
        assert_eq!(env.get("SINGLE"), Some(&"single quotes".to_string()));
    }

    #[test]
    fn test_parse_composer_content() {
        let content = r#"{
            "name": "laravel/laravel",
            "require": {
                "php": "^8.2",
                "laravel/framework": "^11.0"
            },
            "require-dev": {
                "phpunit/phpunit": "^10.0"
            }
        }"#;

        let info = parse_composer_content(content).unwrap();

        assert_eq!(info.name, Some("laravel/laravel".to_string()));
        assert!(info.has_dependency("laravel/framework"));
        assert!(info.has_dependency("phpunit/phpunit"));
        assert_eq!(
            info.get_major_version("laravel/framework"),
            Some("11".to_string())
        );
    }

    #[test]
    fn test_parse_wp_config_content() {
        let content = r#"<?php
define('DB_NAME', 'wordpress');
define('DB_USER', 'wpuser');
define('DB_PASSWORD', 'secret');
define('DB_HOST', 'localhost:3307');
"#;

        let config = parse_wp_config_content(content).unwrap();

        assert_eq!(config.database, "wordpress");
        assert_eq!(config.username, "wpuser");
        assert_eq!(config.password, "secret");
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 3307);
    }

    #[test]
    fn test_update_env_content() {
        let content = "APP_NAME=MyApp\nDB_PORT=3306\n";

        // Update existing
        let updated = update_env_content(content, "DB_PORT", "3330");
        assert!(updated.contains("DB_PORT=3330"));
        assert!(!updated.contains("DB_PORT=3306"));

        // Add new
        let updated = update_env_content(content, "NEW_KEY", "value");
        assert!(updated.contains("NEW_KEY=value"));
    }
}
