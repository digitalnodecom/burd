//! Park Manager
//!
//! Handles the "park" feature similar to Laravel Valet:
//! - Scan parked directories for project subdirectories
//! - Detect project types (PHP Laravel, PHP generic, Static)
//! - Auto-create domains for discovered projects
//! - Generate FrankenPHP Caddyfile for virtual host routing
//! - Support custom drivers via TOML config files

use crate::config::{
    get_instance_dir, ConfigStore, Domain, DomainSource, Instance, ParkedDirectory,
};
use crate::drivers::DriverLoader;
use crate::proxy::ProxyServer;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Global driver loader for custom project detection (cached)
static DRIVER_LOADER: Lazy<Mutex<DriverLoader>> = Lazy::new(|| Mutex::new(DriverLoader::new()));

/// Project type detected from directory contents (Laravel Valet-style detection)
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    /// Laravel project (has artisan file)
    Laravel,
    /// Bedrock WordPress (web/wp structure)
    Bedrock,
    /// CakePHP 3+ (has bin/cake)
    CakePHP,
    /// ConcreteCMS (has concrete/dispatcher.php)
    ConcreteCMS,
    /// Contao CMS
    Contao,
    /// Craft CMS (has craft executable)
    Craft,
    /// Drupal (has core/lib/Drupal.php)
    Drupal,
    /// ExpressionEngine (has system/ee)
    ExpressionEngine,
    /// Jigsaw static site generator
    Jigsaw,
    /// Joomla CMS
    Joomla,
    /// Katana static site generator
    Katana,
    /// Kirby CMS (has kirby folder)
    Kirby,
    /// Magento (1.x or 2.x)
    Magento,
    /// OctoberCMS (Laravel-based with modules/system)
    OctoberCMS,
    /// Sculpin static site generator
    Sculpin,
    /// Slim framework
    Slim,
    /// Statamic (Laravel-based with content folder)
    Statamic,
    /// Symfony (has bin/console or symfony.lock)
    Symfony,
    /// WordPress (has wp-config.php or wp-content)
    WordPress,
    /// Static HTML site
    StaticHtml,
    /// Generic PHP project (has index.php)
    PhpGeneric,
    /// Unknown type
    Unknown,
    /// Custom project type defined by a driver config
    Custom { name: String, requires_php: bool },
}

impl ProjectType {
    pub fn as_str(&self) -> &str {
        match self {
            ProjectType::Laravel => "Laravel",
            ProjectType::Bedrock => "Bedrock",
            ProjectType::CakePHP => "CakePHP",
            ProjectType::ConcreteCMS => "Concrete",
            ProjectType::Contao => "Contao",
            ProjectType::Craft => "Craft",
            ProjectType::Drupal => "Drupal",
            ProjectType::ExpressionEngine => "ExpressionEngine",
            ProjectType::Jigsaw => "Jigsaw",
            ProjectType::Joomla => "Joomla",
            ProjectType::Katana => "Katana",
            ProjectType::Kirby => "Kirby",
            ProjectType::Magento => "Magento",
            ProjectType::OctoberCMS => "October",
            ProjectType::Sculpin => "Sculpin",
            ProjectType::Slim => "Slim",
            ProjectType::Statamic => "Statamic",
            ProjectType::Symfony => "Symfony",
            ProjectType::WordPress => "WordPress",
            ProjectType::StaticHtml => "Static",
            ProjectType::PhpGeneric => "PHP",
            ProjectType::Unknown => "Unknown",
            ProjectType::Custom { name, .. } => name.as_str(),
        }
    }

    /// Check if this project type requires PHP (FrankenPHP)
    pub fn requires_php(&self) -> bool {
        match self {
            ProjectType::StaticHtml | ProjectType::Unknown => false,
            ProjectType::Custom { requires_php, .. } => *requires_php,
            _ => true,
        }
    }
}

/// A discovered project within a parked directory
#[derive(Debug, Clone)]
pub struct DiscoveredProject {
    /// Project name (directory name)
    pub name: String,
    /// Full path to project directory
    pub path: PathBuf,
    /// Detected project type
    pub project_type: ProjectType,
    /// Document root (may be project/public or just project)
    pub document_root: PathBuf,
}

/// Result of syncing a parked directory
#[derive(Debug, Default)]
pub struct SyncResult {
    /// Domains that were added
    pub added: Vec<String>,
    /// Domains that were removed
    pub removed: Vec<String>,
    /// Domains that were skipped due to conflicts
    pub conflicts: Vec<String>,
    /// Number of unchanged domains
    pub unchanged: usize,
    /// Any errors encountered
    pub errors: Vec<String>,
}

/// Directories to skip when scanning parked directories
const SKIP_DIRECTORIES: &[&str] = &[
    "node_modules",
    "vendor",
    ".git",
    ".idea",
    ".vscode",
    "__pycache__",
    ".cache",
    "target",
    "build",
    "dist",
];

/// Scan a directory for project subdirectories
pub fn scan_directory(parked_path: &Path) -> Result<Vec<DiscoveredProject>, String> {
    if !parked_path.exists() {
        return Err(format!("Path '{}' does not exist", parked_path.display()));
    }
    if !parked_path.is_dir() {
        return Err(format!(
            "Path '{}' is not a directory",
            parked_path.display()
        ));
    }

    let entries =
        fs::read_dir(parked_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    let mut projects = Vec::new();

    // Try to acquire driver loader for custom driver detection
    let mut driver_loader = DRIVER_LOADER
        .lock()
        .map_err(|e| format!("Failed to acquire driver loader lock: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden directories
        if name.starts_with('.') {
            continue;
        }

        // Skip common non-project directories
        if SKIP_DIRECTORIES.contains(&name.as_str()) {
            continue;
        }

        // Check custom drivers first, then fall back to built-in detection
        let (project_type, document_root) =
            if let Some(driver_match) = driver_loader.detect_custom(&path) {
                // Custom driver matched
                let project_type = ProjectType::Custom {
                    name: driver_match.name,
                    requires_php: driver_match.requires_php,
                };
                (project_type, driver_match.document_root)
            } else {
                // Use built-in detection
                let project_type = detect_project_type(&path);
                let document_root = determine_document_root(&path, &project_type);
                (project_type, document_root)
            };

        projects.push(DiscoveredProject {
            name,
            path,
            project_type,
            document_root,
        });
    }

    Ok(projects)
}

/// Detect the project type based on directory contents (Laravel Valet-style detection)
pub fn detect_project_type(path: &Path) -> ProjectType {
    // === MOST SPECIFIC FIRST (Laravel variants before Laravel) ===

    // Statamic (Laravel-based with content folder or statamic folder)
    if path.join("artisan").exists()
        && (path.join("content").is_dir() || path.join("statamic").is_dir())
    {
        return ProjectType::Statamic;
    }

    // OctoberCMS (Laravel-based with modules/system)
    if path.join("artisan").exists() && path.join("modules/system").is_dir() {
        return ProjectType::OctoberCMS;
    }

    // Laravel - has artisan file (after Statamic/October checks)
    if path.join("artisan").exists() {
        return ProjectType::Laravel;
    }

    // Bedrock - WordPress with modern structure (web/wp/)
    if path.join("web/wp/wp-settings.php").exists() {
        return ProjectType::Bedrock;
    }

    // Symfony - has bin/console or symfony.lock
    if path.join("bin/console").exists() || path.join("symfony.lock").exists() {
        return ProjectType::Symfony;
    }

    // CakePHP 3+ - has bin/cake or config/app.php with CakePHP markers
    if path.join("bin/cake").exists()
        || (path.join("config/app.php").exists() && path.join("src/Controller").is_dir())
    {
        return ProjectType::CakePHP;
    }

    // Craft CMS - has craft executable with config directory
    if path.join("craft").exists() && path.join("config").is_dir() {
        return ProjectType::Craft;
    }

    // Drupal - has core/lib/Drupal.php
    if path.join("core/lib/Drupal.php").exists() {
        return ProjectType::Drupal;
    }

    // Joomla - has administrator folder with index.php
    if path.join("administrator/index.php").exists() && path.join("libraries/joomla").is_dir() {
        return ProjectType::Joomla;
    }

    // Magento - has app/Mage.php (1.x) or bin/magento (2.x)
    if path.join("bin/magento").exists() || path.join("app/Mage.php").exists() {
        return ProjectType::Magento;
    }

    // Kirby - has kirby folder
    if path.join("kirby").is_dir() {
        return ProjectType::Kirby;
    }

    // ExpressionEngine - has system/ee folder
    if path.join("system/ee").is_dir() {
        return ProjectType::ExpressionEngine;
    }

    // ConcreteCMS - has concrete/dispatcher.php
    if path.join("concrete/dispatcher.php").exists() {
        return ProjectType::ConcreteCMS;
    }

    // Contao - has contao-manager.phar.php or vendor/contao
    if path.join("contao-manager.phar.php").exists() || path.join("vendor/contao").is_dir() {
        return ProjectType::Contao;
    }

    // Sculpin - has sculpin.json
    if path.join("sculpin.json").exists() {
        return ProjectType::Sculpin;
    }

    // Jigsaw - has config.php with source directory (blade templates)
    if path.join("config.php").exists() && path.join("source").is_dir() {
        return ProjectType::Jigsaw;
    }

    // Katana - has katana file
    if path.join("katana").exists() {
        return ProjectType::Katana;
    }

    // Slim - has vendor/slim/slim
    if path.join("vendor/slim/slim").is_dir() {
        return ProjectType::Slim;
    }

    // WordPress - has wp-config.php or wp-content directory
    if path.join("wp-config.php").exists() || path.join("wp-content").is_dir() {
        return ProjectType::WordPress;
    }

    // === GENERIC FALLBACKS ===

    // Generic PHP with public directory
    if path.join("public/index.php").exists() {
        return ProjectType::PhpGeneric;
    }

    // Generic PHP at root
    if path.join("index.php").exists() {
        return ProjectType::PhpGeneric;
    }

    // Static with public directory
    if path.join("public/index.html").exists() {
        return ProjectType::StaticHtml;
    }

    // Static at root
    if path.join("index.html").exists() {
        return ProjectType::StaticHtml;
    }

    ProjectType::Unknown
}

/// Determine the document root for a project
pub fn determine_document_root(path: &Path, project_type: &ProjectType) -> PathBuf {
    match project_type {
        // Laravel-family: always public/
        ProjectType::Laravel | ProjectType::Statamic | ProjectType::OctoberCMS => {
            path.join("public")
        }

        // Symfony/modern PHP: public/
        ProjectType::Symfony | ProjectType::CakePHP | ProjectType::Slim => path.join("public"),

        // Craft: web/
        ProjectType::Craft => path.join("web"),

        // Bedrock: web/
        ProjectType::Bedrock => path.join("web"),

        // Jigsaw: build_local or build_production
        ProjectType::Jigsaw => {
            if path.join("build_production").is_dir() {
                path.join("build_production")
            } else {
                path.join("build_local")
            }
        }

        // Sculpin: output_dev or output_prod
        ProjectType::Sculpin => {
            if path.join("output_prod").is_dir() {
                path.join("output_prod")
            } else {
                path.join("output_dev")
            }
        }

        // Katana: public/
        ProjectType::Katana => path.join("public"),

        // Kirby: prefer public/ if it exists
        ProjectType::Kirby => {
            let public = path.join("public");
            if public.is_dir() {
                public
            } else {
                path.to_path_buf()
            }
        }

        // CMS platforms: serve from root
        ProjectType::WordPress
        | ProjectType::Drupal
        | ProjectType::Joomla
        | ProjectType::Magento
        | ProjectType::ExpressionEngine
        | ProjectType::ConcreteCMS
        | ProjectType::Contao => path.to_path_buf(),

        // Generic PHP: prefer public/ if it has index.php
        ProjectType::PhpGeneric => {
            let public = path.join("public");
            if public.join("index.php").exists() {
                public
            } else {
                path.to_path_buf()
            }
        }

        // Static: prefer public/ if it has index.html
        ProjectType::StaticHtml => {
            let public = path.join("public");
            if public.join("index.html").exists() {
                public
            } else {
                path.to_path_buf()
            }
        }

        // Unknown: just use project root
        ProjectType::Unknown => path.to_path_buf(),

        // Custom: document root is determined by the driver, but fallback to project root
        // This branch shouldn't be reached normally since custom types use driver's doc root
        ProjectType::Custom { .. } => path.to_path_buf(),
    }
}

/// Generate a subdomain slug from a project name
pub fn generate_subdomain(name: &str) -> String {
    slug::slugify(name)
}

/// Sync domains for a parked directory
/// Creates new domains for discovered projects, removes orphaned domains
pub fn sync_parked_domains(
    parked_dir: &ParkedDirectory,
    config_store: &ConfigStore,
    proxy: &ProxyServer,
    tld: &str,
) -> Result<SyncResult, String> {
    let mut result = SyncResult::default();

    // Get the park instance
    let park_instance = config_store.get_park_instance()?;
    let park_port = match &park_instance {
        Some(instance) => instance.port,
        None => {
            return Err("FrankenPHP Park instance not found. Please create one first.".to_string());
        }
    };

    // Scan the parked directory
    let discovered = scan_directory(Path::new(&parked_dir.path))?;

    // Load current config
    let config = config_store.load()?;

    // Get current parked domains for this directory
    let current_parked_domains: HashSet<String> = config
        .domains
        .iter()
        .filter(|d| d.parked_dir_id() == Some(parked_dir.id))
        .map(|d| d.subdomain.clone())
        .collect();

    // Get all existing subdomains (for conflict detection)
    let all_existing_subdomains: HashSet<String> =
        config.domains.iter().map(|d| d.subdomain.clone()).collect();

    // Discovered subdomain names
    let discovered_names: HashSet<String> = discovered
        .iter()
        .map(|p| generate_subdomain(&p.name))
        .collect();

    // Add new projects
    for project in &discovered {
        let subdomain = generate_subdomain(&project.name);

        // Check if this subdomain already exists for this parked directory
        if current_parked_domains.contains(&subdomain) {
            result.unchanged += 1;
            continue;
        }

        // Check for conflict with other domains
        if all_existing_subdomains.contains(&subdomain) {
            result.conflicts.push(subdomain);
            continue;
        }

        // All parked projects go through FrankenPHP Park
        // (FrankenPHP Park handles both PHP and static files via its Caddyfile)
        let domain = Domain::for_parked_port(
            subdomain.clone(),
            park_port,
            parked_dir.ssl_enabled,
            parked_dir.id,
        );

        // Save domain and register route
        let mut config = config_store.load()?;
        config.domains.push(domain.clone());
        config_store.save(&config)?;

        // Register with proxy - all parked domains go through FrankenPHP Park port
        let full_domain = domain.full_domain(tld);
        let _ = proxy.register_route(
            &full_domain,
            park_port,
            &domain.id.to_string(),
            domain.ssl_enabled,
        );

        result.added.push(subdomain);
    }

    // Remove orphaned domains (folder was deleted)
    let orphaned: Vec<String> = current_parked_domains
        .difference(&discovered_names)
        .cloned()
        .collect();

    for subdomain in orphaned {
        // Find and remove domain
        let mut config = config_store.load()?;
        if let Some(idx) = config
            .domains
            .iter()
            .position(|d| d.subdomain == subdomain && d.parked_dir_id() == Some(parked_dir.id))
        {
            let domain = config.domains.remove(idx);
            config_store.save(&config)?;

            // Unregister from proxy
            let full_domain = domain.full_domain(tld);
            let _ = proxy.unregister_route(&full_domain);

            result.removed.push(subdomain);
        }
    }

    // Regenerate FrankenPHP Park Caddyfile if there were changes
    if !result.added.is_empty() || !result.removed.is_empty() {
        if let Some(instance) = &park_instance {
            if let Err(e) = regenerate_park_caddyfile(config_store, instance, tld) {
                result
                    .errors
                    .push(format!("Failed to regenerate Caddyfile: {}", e));
            }
        }
    }

    Ok(result)
}

/// Regenerate the FrankenPHP Park Caddyfile with all parked projects (PHP and static)
pub fn regenerate_park_caddyfile(
    config_store: &ConfigStore,
    park_instance: &Instance,
    tld: &str,
) -> Result<(), String> {
    let config = config_store.load()?;

    // Collect all parked projects (both PHP and static)
    // (domain, document_root, requires_php)
    let mut all_projects: Vec<(String, String, bool)> = Vec::new();

    for parked_dir in &config.parked_directories {
        let projects = scan_directory(Path::new(&parked_dir.path))?;

        for project in projects {
            let subdomain = generate_subdomain(&project.name);

            // Check if domain exists and is not isolated
            let domain_exists = config.domains.iter().any(|d| {
                d.subdomain == subdomain && matches!(d.source, DomainSource::Parked { .. })
            });

            if domain_exists {
                all_projects.push((
                    format!("{}.{}", subdomain, tld),
                    project.document_root.to_string_lossy().to_string(),
                    project.project_type.requires_php(),
                ));
            }
        }
    }

    // Generate Caddyfile content
    let caddyfile = generate_caddyfile_content(&all_projects, park_instance.port);

    // Write to instance directory
    let instance_dir = get_instance_dir(&park_instance.id)?;
    fs::create_dir_all(&instance_dir)
        .map_err(|e| format!("Failed to create instance directory: {}", e))?;

    let caddyfile_path = instance_dir.join("Caddyfile");
    fs::write(&caddyfile_path, &caddyfile)
        .map_err(|e| format!("Failed to write Caddyfile: {}", e))?;

    Ok(())
}

/// Generate the Caddyfile content for FrankenPHP Park
/// Projects tuple: (domain, document_root, requires_php)
fn generate_caddyfile_content(projects: &[(String, String, bool)], port: u16) -> String {
    let mut content = String::new();

    // Global options
    content.push_str("{\n");
    content.push_str("    frankenphp\n");
    content.push_str("    order php_server before file_server\n");
    content.push_str("}\n\n");

    // Server block for the port
    content.push_str(&format!(":{} {{\n", port));

    // Map X-Forwarded-Proto header to HTTPS env value
    // This allows PHP to correctly detect HTTPS when behind Caddy reverse proxy
    content.push_str("    map {header.X-Forwarded-Proto} {https_env} {\n");
    content.push_str("        https on\n");
    content.push_str("        default \"\"\n");
    content.push_str("    }\n\n");

    // Add handler for each project
    for (domain, document_root, requires_php) in projects {
        let matcher_name = domain.replace(['.', '-'], "_");
        content.push_str(&format!("    @{} host {}\n", matcher_name, domain));
        content.push_str(&format!("    handle @{} {{\n", matcher_name));
        // Quote the path to handle spaces and special characters
        content.push_str(&format!("        root * \"{}\"\n", document_root));
        if *requires_php {
            // PHP projects use php_server with HTTPS env and try_files for pretty URLs
            content.push_str("        php_server {\n");
            content.push_str("            env HTTPS {https_env}\n");
            content.push_str("        }\n");
            content.push_str("        try_files {path} {path}/ /index.php?{query}\n");
        } else {
            // Static projects use file_server
            content.push_str("        file_server\n");
        }
        content.push_str("    }\n\n");
    }

    // Fallback handler
    content.push_str("    handle {\n");
    content.push_str("        respond \"Site not found. Make sure your project is in a parked directory.\" 404\n");
    content.push_str("    }\n");

    content.push_str("}\n");

    content
}

/// Get all projects across all parked directories
pub fn get_all_parked_projects(
    config_store: &ConfigStore,
) -> Result<Vec<(ParkedDirectory, Vec<DiscoveredProject>)>, String> {
    let config = config_store.load()?;
    let mut all_projects = Vec::new();

    for parked_dir in config.parked_directories {
        let projects = scan_directory(Path::new(&parked_dir.path))?;
        all_projects.push((parked_dir, projects));
    }

    Ok(all_projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_subdomain() {
        assert_eq!(generate_subdomain("my-project"), "my-project");
        assert_eq!(generate_subdomain("My Project"), "my-project");
        assert_eq!(generate_subdomain("test_project"), "test-project");
    }

    #[test]
    fn test_generate_caddyfile_content() {
        let projects = vec![
            (
                "blog.burd".to_string(),
                "/Users/dev/Sites/blog/public".to_string(),
                true,
            ), // PHP
            (
                "api.burd".to_string(),
                "/Users/dev/Sites/api/public".to_string(),
                true,
            ), // PHP
            (
                "docs.burd".to_string(),
                "/Users/dev/Sites/docs".to_string(),
                false,
            ), // Static
        ];
        let content = generate_caddyfile_content(&projects, 8888);

        assert!(content.contains("frankenphp"));
        assert!(content.contains(":8888"));
        assert!(content.contains("@blog_burd host blog.burd"));
        assert!(content.contains("root * \"/Users/dev/Sites/blog/public\""));
        assert!(content.contains("php_server"));
        assert!(content.contains("file_server")); // Static project uses file_server
    }
}
