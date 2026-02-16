//! Project type detection
//!
//! Logic for detecting what type of project is in a directory.

use super::parsers::parse_composer_json;
use super::types::ProjectType;
use std::path::Path;

/// Detect the project type from a directory path
///
/// Checks for various indicators in the following priority order:
/// 1. Laravel (artisan + laravel/framework dependency)
/// 2. Bedrock (web/wp/ directory or config/application.php)
/// 3. WordPress (wp-config.php or wp-content/)
/// 4. Symfony (symfony/framework-bundle dependency)
/// 5. Unknown
pub fn detect_project_type(path: &Path) -> ProjectType {
    // Check for Laravel first (most specific)
    if let Some(laravel) = detect_laravel(path) {
        return laravel;
    }

    // Check for Bedrock (before generic WordPress)
    if detect_bedrock(path) {
        return ProjectType::Bedrock;
    }

    // Check for standard WordPress
    if detect_wordpress(path) {
        return ProjectType::WordPress;
    }

    // Check for Symfony
    if let Some(symfony) = detect_symfony(path) {
        return symfony;
    }

    ProjectType::Unknown
}

/// Detect Laravel project
fn detect_laravel(path: &Path) -> Option<ProjectType> {
    // Must have artisan file
    if !path.join("artisan").exists() {
        return None;
    }

    // Check composer.json for laravel/framework
    if let Some(composer) = parse_composer_json(path) {
        if composer.has_dependency("laravel/framework") {
            let version = composer.get_major_version("laravel/framework");
            return Some(ProjectType::Laravel { version });
        }
    }

    // Has artisan but no laravel/framework in composer - could be Lumen or custom
    // Still treat as Laravel-ish
    Some(ProjectType::Laravel { version: None })
}

/// Detect Bedrock project
fn detect_bedrock(path: &Path) -> bool {
    // Bedrock uses web/wp/ for WordPress core
    if path.join("web/wp").is_dir() {
        return true;
    }

    // Or has config/application.php (Bedrock's main config)
    if path.join("config/application.php").exists() {
        return true;
    }

    // Check composer.json for roots/bedrock or roots/wordpress
    if let Some(composer) = parse_composer_json(path) {
        if composer.has_dependency("roots/bedrock")
            || composer.has_dependency("roots/wordpress")
            || composer.has_dependency("johnpbloch/wordpress-core")
        {
            // Also check for Bedrock structure
            if path.join("web").is_dir() || path.join("config").is_dir() {
                return true;
            }
        }
    }

    false
}

/// Detect standard WordPress
fn detect_wordpress(path: &Path) -> bool {
    // Has wp-config.php in root
    if path.join("wp-config.php").exists() {
        return true;
    }

    // Has wp-config-sample.php (fresh install)
    if path.join("wp-config-sample.php").exists() {
        return true;
    }

    // Has wp-content directory (WordPress structure)
    if path.join("wp-content").is_dir() && path.join("wp-includes").is_dir() {
        return true;
    }

    false
}

/// Detect Symfony project
fn detect_symfony(path: &Path) -> Option<ProjectType> {
    // Check for Symfony console
    let has_console = path.join("bin/console").exists();

    // Check composer.json for Symfony framework
    if let Some(composer) = parse_composer_json(path) {
        if composer.has_dependency("symfony/framework-bundle")
            || composer.has_dependency("symfony/symfony")
        {
            let version = composer
                .get_major_version("symfony/framework-bundle")
                .or_else(|| composer.get_major_version("symfony/symfony"));
            return Some(ProjectType::Symfony { version });
        }

        // Has console and some Symfony components
        if has_console && composer.has_dependency("symfony/console") {
            return Some(ProjectType::Symfony { version: None });
        }
    }

    None
}

/// Get the document root for a project based on its type
///
/// Different frameworks have different conventions for where web files live:
/// - Laravel: public/
/// - Bedrock: web/
/// - WordPress: root directory
/// - Symfony: public/
pub fn get_document_root(path: &Path, project_type: &ProjectType) -> std::path::PathBuf {
    match project_type {
        ProjectType::Laravel { .. } => {
            let public = path.join("public");
            if public.is_dir() {
                public
            } else {
                path.to_path_buf()
            }
        }
        ProjectType::Bedrock => {
            let web = path.join("web");
            if web.is_dir() {
                web
            } else {
                path.to_path_buf()
            }
        }
        ProjectType::WordPress => path.to_path_buf(),
        ProjectType::Symfony { .. } => {
            let public = path.join("public");
            if public.is_dir() {
                public
            } else {
                path.to_path_buf()
            }
        }
        ProjectType::Unknown => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_temp_project() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    #[test]
    fn test_detect_laravel() {
        let temp = create_temp_project();
        let path = temp.path();

        // Create artisan file
        fs::write(path.join("artisan"), "#!/usr/bin/env php").unwrap();

        // Create composer.json with laravel/framework
        fs::write(
            path.join("composer.json"),
            r#"{"require": {"laravel/framework": "^11.0"}}"#,
        )
        .unwrap();

        // Create public directory
        fs::create_dir(path.join("public")).unwrap();

        let detected = detect_project_type(path);
        assert!(matches!(detected, ProjectType::Laravel { version: Some(v) } if v == "11"));
    }

    #[test]
    fn test_detect_wordpress() {
        let temp = create_temp_project();
        let path = temp.path();

        // Create wp-config.php
        fs::write(path.join("wp-config.php"), "<?php // WordPress").unwrap();

        let detected = detect_project_type(path);
        assert!(matches!(detected, ProjectType::WordPress));
    }

    #[test]
    fn test_detect_bedrock() {
        let temp = create_temp_project();
        let path = temp.path();

        // Create Bedrock structure
        fs::create_dir_all(path.join("web/wp")).unwrap();
        fs::create_dir_all(path.join("config")).unwrap();
        fs::write(path.join("config/application.php"), "<?php").unwrap();

        let detected = detect_project_type(path);
        assert!(matches!(detected, ProjectType::Bedrock));
    }

    #[test]
    fn test_detect_unknown() {
        let temp = create_temp_project();
        let path = temp.path();

        // Empty directory
        let detected = detect_project_type(path);
        assert!(matches!(detected, ProjectType::Unknown));
    }
}
