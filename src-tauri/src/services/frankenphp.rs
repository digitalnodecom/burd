use crate::config::{get_instance_dir, Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct FrankenPHPService;

/// Generate framework-specific Caddyfile blocks based on the "framework" config key.
/// Returns (extra_directives_inside_site_block, php_server_body) tuple.
fn framework_directives(instance: &Instance) -> (&'static str, &'static str) {
    let framework = instance
        .config
        .get("framework")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    match framework {
        // Magento 2: needs custom /static/ and /media/ handling
        "magento2" => (
            r#"
    # Magento 2: strip version prefix from static URLs
    @magentoStatic path /static/*
    handle @magentoStatic {
        @versioned path_regexp static /static/version\d+/(.*)
        rewrite @versioned /static/{re.static.1}
        @staticMissing {
            not file
            path *.php
        }
        rewrite @staticMissing /static.php?resource={path}&{query}
        file_server
        php
    }

    # Magento 2: media fallback to get.php
    @magentoMedia path /media/*
    handle @magentoMedia {
        @mediaMissing not file
        rewrite @mediaMissing /get.php?resource={path}&{query}
        file_server
        php
    }
"#,
            // Default php_server handles the rest
            "        env HTTPS {https_env}\n",
        ),

        // Drupal: security hardening to block sensitive files
        "drupal" => (
            r#"
    # Drupal: block access to sensitive files
    @drupalForbidden {
        path *.engine *.inc *.install *.module *.profile *.po *.sh *.theme
        path /vendor/* /node_modules/*
        path /sites/*/files/private/*
    }
    error @drupalForbidden 403
"#,
            "        env HTTPS {https_env}\n",
        ),

        // Symfony: block non-front-controller PHP files
        "symfony" => (
            r#"
    # Symfony: block direct access to non-front-controller PHP files
    @symfonyBlockPhp {
        path *.php
        not path /index.php
    }
    error @symfonyBlockPhp 404
"#,
            "        env HTTPS {https_env}\n",
        ),

        // Default: bare php_server handles Laravel, WordPress, Bedrock, OpenCart, etc.
        _ => ("", "        env HTTPS {https_env}\n"),
    }
}

impl ServiceDefinition for FrankenPHPService {
    fn service_type(&self) -> ServiceType {
        ServiceType::FrankenPHP
    }

    fn display_name(&self) -> &'static str {
        "PHP"
    }

    fn default_port(&self) -> u16 {
        8000
    }

    fn binary_name(&self) -> &'static str {
        "frankenphp"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::GitHubReleases("https://api.github.com/repos/dunglas/frankenphp/releases")
    }

    fn download_method(&self, _version: &str, arch: &str) -> DownloadMethod {
        let asset_pattern = if arch == "aarch64" {
            "frankenphp-mac-arm64"
        } else {
            "frankenphp-mac-x86_64"
        };
        DownloadMethod::GitHubRelease {
            api_url: "https://api.github.com/repos/dunglas/frankenphp/releases/tags/",
            asset_pattern: asset_pattern.to_string(),
            checksum: None,
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    fn start_args(&self, instance: &Instance, _data_dir: &Path) -> Vec<String> {
        let data_dir =
            get_instance_dir(&instance.id).unwrap_or_else(|_| std::path::PathBuf::from("."));

        let config_file = data_dir.join("Caddyfile");
        let custom_caddyfile = data_dir.join("Caddyfile.custom");

        // If user provides a custom Caddyfile, use it directly (Burd won't overwrite)
        if !custom_caddyfile.exists() {
            // Get document root from config, default to current directory
            let doc_root = instance
                .config
                .get("document_root")
                .and_then(|v| v.as_str())
                .unwrap_or(".");

            let (extra_directives, php_server_body) = framework_directives(instance);

            // Generate Caddyfile — uses bare php_server which handles all major PHP
            // frameworks by default (try_files {path} {path}/index.php index.php)
            let caddyfile = format!(
                r#"{{
    frankenphp
    order php_server before file_server
}}

:{port} {{
    # Map X-Forwarded-Proto header to HTTPS env value
    map {{header.X-Forwarded-Proto}} {{https_env}} {{
        https on
        default ""
    }}

    root * "{doc_root}"

    # Enable compression (zstd, brotli, gzip)
    encode zstd br gzip
{extra_directives}
    # PHP routing — default php_server handles Laravel, WordPress, Bedrock,
    # OpenCart, Symfony, Drupal, and most PHP frameworks out of the box
    php_server {{
{php_server_body}    }}
}}
"#,
                port = instance.port,
                doc_root = doc_root,
                extra_directives = extra_directives,
                php_server_body = php_server_body
            );

            let _ = std::fs::write(&config_file, caddyfile);
        } else {
            // Copy custom Caddyfile to the expected location
            let _ = std::fs::copy(&custom_caddyfile, &config_file);
        }

        vec![
            "run".to_string(),
            "--config".to_string(),
            config_file.to_string_lossy().to_string(),
        ]
    }

    fn env_vars(&self, instance: &Instance, domain: Option<&str>) -> Vec<(String, String)> {
        let mut vars = vec![];

        // Set SERVER_NAME to the domain if available (important for PHP apps like Laravel)
        if let Some(d) = domain {
            vars.push(("SERVER_NAME".to_string(), d.to_string()));
        }

        // PHP memory limit
        if let Some(v) = instance
            .config
            .get("php_memory_limit")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_MEMORY_LIMIT".to_string(), v.to_string()));
            }
        }

        // PHP upload max filesize
        if let Some(v) = instance
            .config
            .get("php_upload_max_filesize")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_UPLOAD_MAX_FILESIZE".to_string(), v.to_string()));
            }
        }

        // PHP post max size
        if let Some(v) = instance
            .config
            .get("php_post_max_size")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_POST_MAX_SIZE".to_string(), v.to_string()));
            }
        }

        // PHP max execution time
        if let Some(v) = instance
            .config
            .get("php_max_execution_time")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_MAX_EXECUTION_TIME".to_string(), v.to_string()));
            }
        }

        vars
    }
}
