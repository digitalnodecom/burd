//! New project CLI command
//!
//! Creates new projects from templates (Laravel, WordPress, Bedrock).

use std::env;
use std::path::Path;
use std::process::Command;

/// Supported project types for scaffolding
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectTemplate {
    Laravel,
    WordPress,
    Bedrock,
}

impl ProjectTemplate {
    /// Parse template from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "laravel" => Some(Self::Laravel),
            "wordpress" | "wp" => Some(Self::WordPress),
            "bedrock" => Some(Self::Bedrock),
            _ => None,
        }
    }

    /// Get display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Laravel => "Laravel",
            Self::WordPress => "WordPress",
            Self::Bedrock => "Bedrock",
        }
    }
}

/// Create a new project
///
/// Scaffolds a new project using composer or direct download.
pub fn run_new(template: &str, name: &str) -> Result<(), String> {
    let project_type = ProjectTemplate::parse(template).ok_or_else(|| {
        format!(
            "Unknown project type: '{}'\n\n\
             Supported types:\n  \
             - laravel    Laravel PHP framework\n  \
             - wordpress  Standard WordPress installation\n  \
             - bedrock    Roots Bedrock WordPress",
            template
        )
    })?;

    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let project_dir = current_dir.join(name);

    // Check if directory already exists
    if project_dir.exists() {
        return Err(format!(
            "Directory '{}' already exists.\nChoose a different name or remove the existing directory.",
            name
        ));
    }

    println!();
    println!("Creating {} project '{}'...", project_type.display_name(), name);
    println!();

    // Create the project based on type
    match project_type {
        ProjectTemplate::Laravel => create_laravel_project(&project_dir, name)?,
        ProjectTemplate::WordPress => create_wordpress_project(&project_dir, name)?,
        ProjectTemplate::Bedrock => create_bedrock_project(&project_dir, name)?,
    }

    println!();
    println!("Project created successfully!");
    println!();
    println!("Next steps:");
    println!("  cd {}", name);
    println!("  burd link");
    println!();

    Ok(())
}

/// Create a new Laravel project using composer
fn create_laravel_project(target: &Path, name: &str) -> Result<(), String> {
    // Check if composer is available
    check_composer()?;

    println!("Running: composer create-project laravel/laravel {}", name);
    println!();

    let status = Command::new("composer")
        .args([
            "create-project",
            "--prefer-dist",
            "laravel/laravel",
            target.to_str().ok_or("Invalid path")?,
        ])
        .status()
        .map_err(|e| format!("Failed to run composer: {}", e))?;

    if !status.success() {
        return Err("Failed to create Laravel project. Check composer output above.".to_string());
    }

    println!();
    println!("Laravel project created.");

    // Generate app key
    println!("Generating application key...");
    let key_status = Command::new("php")
        .args(["artisan", "key:generate"])
        .current_dir(target)
        .status()
        .map_err(|e| format!("Failed to generate app key: {}", e))?;

    if !key_status.success() {
        eprintln!("Warning: Failed to generate application key. Run 'php artisan key:generate' manually.");
    }

    Ok(())
}

/// Create a new WordPress project by downloading from wordpress.org
fn create_wordpress_project(target: &Path, name: &str) -> Result<(), String> {
    // Check if curl/wget is available
    let has_curl = Command::new("which")
        .arg("curl")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_curl {
        return Err("curl is required to download WordPress. Please install it first.".to_string());
    }

    // Create target directory
    std::fs::create_dir_all(target)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let archive_path = target.join("wordpress.tar.gz");

    // Download WordPress
    println!("Downloading WordPress...");
    let download_status = Command::new("curl")
        .args([
            "-L",
            "-o",
            archive_path.to_str().ok_or("Invalid path")?,
            "https://wordpress.org/latest.tar.gz",
        ])
        .status()
        .map_err(|e| format!("Failed to download WordPress: {}", e))?;

    if !download_status.success() {
        return Err("Failed to download WordPress.".to_string());
    }

    // Extract archive
    println!("Extracting...");
    let extract_status = Command::new("tar")
        .args([
            "-xzf",
            archive_path.to_str().ok_or("Invalid path")?,
            "-C",
            target.to_str().ok_or("Invalid path")?,
            "--strip-components=1",
        ])
        .status()
        .map_err(|e| format!("Failed to extract WordPress: {}", e))?;

    if !extract_status.success() {
        return Err("Failed to extract WordPress archive.".to_string());
    }

    // Remove archive
    let _ = std::fs::remove_file(&archive_path);

    // Create wp-config.php from sample
    let sample_config = target.join("wp-config-sample.php");
    let config_path = target.join("wp-config.php");

    if sample_config.exists() {
        println!("Creating wp-config.php...");

        let mut content = std::fs::read_to_string(&sample_config)
            .map_err(|e| format!("Failed to read wp-config-sample.php: {}", e))?;

        // Replace database settings with placeholders that make sense
        content = content.replace("database_name_here", name);
        content = content.replace("username_here", "root");
        content = content.replace("password_here", "");
        content = content.replace("localhost", "127.0.0.1");

        // Generate unique keys (simple version - WordPress will regenerate on first load)
        let unique_phrase = format!("burd-{}-{}", name, std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs());

        content = content.replace("put your unique phrase here", &unique_phrase);

        std::fs::write(&config_path, content)
            .map_err(|e| format!("Failed to write wp-config.php: {}", e))?;
    }

    println!();
    println!("WordPress installed.");
    println!();
    println!("Configure database settings in wp-config.php or run 'burd link' for automatic setup.");

    Ok(())
}

/// Create a new Bedrock project using composer
fn create_bedrock_project(target: &Path, name: &str) -> Result<(), String> {
    // Check if composer is available
    check_composer()?;

    println!("Running: composer create-project roots/bedrock {}", name);
    println!();

    let status = Command::new("composer")
        .args([
            "create-project",
            "--prefer-dist",
            "roots/bedrock",
            target.to_str().ok_or("Invalid path")?,
        ])
        .status()
        .map_err(|e| format!("Failed to run composer: {}", e))?;

    if !status.success() {
        return Err("Failed to create Bedrock project. Check composer output above.".to_string());
    }

    println!();
    println!("Bedrock project created.");

    // Copy .env.example to .env if it exists
    let env_example = target.join(".env.example");
    let env_path = target.join(".env");

    if env_example.exists() && !env_path.exists() {
        println!("Creating .env from .env.example...");

        let mut content = std::fs::read_to_string(&env_example)
            .map_err(|e| format!("Failed to read .env.example: {}", e))?;

        // Update database name
        content = content.replace("DB_NAME='database_name'", &format!("DB_NAME='{}'", name));
        content = content.replace("DB_NAME=database_name", &format!("DB_NAME={}", name));

        // Set WP_HOME
        content = content.replace(
            "WP_HOME='http://example.com'",
            &format!("WP_HOME='https://{}.test'", slug::slugify(name)),
        );
        content = content.replace(
            "WP_HOME=http://example.com",
            &format!("WP_HOME=https://{}.test", slug::slugify(name)),
        );

        std::fs::write(&env_path, content)
            .map_err(|e| format!("Failed to write .env: {}", e))?;
    }

    Ok(())
}

/// Check if composer is available
fn check_composer() -> Result<(), String> {
    let result = Command::new("composer")
        .arg("--version")
        .output()
        .map_err(|_| {
            "Composer is not installed or not in PATH.\n\n\
             Install it from: https://getcomposer.org/download/\n\
             Or with Homebrew: brew install composer"
                .to_string()
        })?;

    if !result.status.success() {
        return Err("Composer is not working properly.".to_string());
    }

    Ok(())
}
