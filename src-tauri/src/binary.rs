//! Binary Manager
//!
//! Downloads, installs, and manages service binaries (Caddy, FrankenPHP, etc.).
//! Handles version management, download progress reporting, and binary verification.

use crate::config::{
    get_bin_dir, get_binary_name, get_binary_path, get_service_bin_dir,
    get_versioned_binary_dir, BinaryInfo, ConfigStore, ServiceType,
};
use crate::service_config::{
    get_current_platform, DownloadConfig, ServiceRegistry, VersionConfig,
};
use crate::services::{get_service, DownloadMethod, VersionSource};
use serde::Serialize;
use chrono::Utc;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Sha256, Digest};
use std::fs::{self, File};
use std::io::{Write, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tar::Archive;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    assets: Vec<GitHubAsset>,
    prerelease: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub version: String,
    pub is_latest: bool,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadProgress {
    pub service_type: String,
    pub downloaded: u64,
    pub total: u64,
    pub percentage: f64,
    /// Phase: "downloading", "extracting", or "installing" (for homebrew)
    pub phase: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BinaryStatus {
    pub service_type: String,
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

/// Verify the SHA256 checksum of a downloaded file
///
/// # Arguments
/// * `file_path` - Path to the file to verify
/// * `expected_checksum` - Expected SHA256 checksum as a hex string
///
/// # Returns
/// * `Ok(())` if checksum matches
/// * `Err(String)` with error message if verification fails
fn verify_checksum(file_path: &Path, expected_checksum: &str) -> Result<(), String> {
    // Read the file
    let mut file = File::open(file_path)
        .map_err(|e| format!("Failed to open file for checksum verification: {}", e))?;

    // Compute SHA256 hash
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 8192]; // 8KB buffer for reading
    loop {
        let bytes_read = file.read(&mut buffer)
            .map_err(|e| format!("Failed to read file for checksum: {}", e))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let computed_hash = hasher.finalize();
    let computed_hex = format!("{:x}", computed_hash);

    // Compare checksums (case-insensitive)
    if computed_hex.eq_ignore_ascii_case(expected_checksum) {
        Ok(())
    } else {
        Err(format!(
            "Checksum verification failed!\nExpected: {}\nComputed: {}\nThe downloaded file may be corrupted or tampered with.",
            expected_checksum,
            computed_hex
        ))
    }
}

#[derive(Clone)]
pub struct BinaryManager {
    client: Client,
}

impl BinaryManager {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn get_status_sync(
        &self,
        service_type: ServiceType,
        config_store: &ConfigStore,
    ) -> Result<BinaryStatus, String> {
        // Get installed versions for this service
        let installed_versions = self.get_installed_versions_sync(service_type)?;
        let installed = !installed_versions.is_empty();

        // Get version info from config (for display)
        let binary_info = config_store.get_binary_info(service_type, None)?;

        Ok(BinaryStatus {
            service_type: service_type.as_str().to_string(),
            installed,
            version: binary_info.as_ref().map(|b| b.version.clone()),
            path: binary_info.as_ref().map(|info| info.path.clone()),
        })
    }

    /// Get all installed versions for a service by scanning the versioned directory
    pub fn get_installed_versions_sync(&self, service_type: ServiceType) -> Result<Vec<String>, String> {
        let service_dir = get_service_bin_dir(service_type)?;
        let legacy_path = get_binary_path(service_type)?;

        // First check for legacy flat binary (old structure: bin/meilisearch as a file)
        // In the old structure, there was no service_dir - just a flat binary file
        // The service_dir path (bin/meilisearch/) wouldn't exist, but the legacy_path (bin/meilisearch) would
        if legacy_path.exists() && legacy_path.is_file() {
            // Legacy binary exists at flat path - return "legacy" as version
            return Ok(vec!["legacy".to_string()]);
        }

        // Check if the service_dir path exists but is actually a file (shouldn't happen but handle it)
        if service_dir.exists() && !service_dir.is_dir() {
            // This would be weird but handle it
            return Ok(vec!["legacy".to_string()]);
        }

        if !service_dir.exists() {
            return Ok(vec![]);
        }

        let binary_name = get_binary_name(service_type);
        let mut versions = Vec::new();

        for entry in fs::read_dir(&service_dir)
            .map_err(|e| format!("Failed to read service directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                let version = path.file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.to_string());

                if let Some(ver) = version {
                    // Verify the binary exists in this version directory
                    if path.join(binary_name).exists() {
                        versions.push(ver);
                    }
                }
            }
        }

        // Sort versions (newest first, simple string sort works for semver)
        versions.sort_by(|a, b| b.cmp(a));

        Ok(versions)
    }

    pub fn get_all_statuses_sync(
        &self,
        config_store: &ConfigStore,
    ) -> Result<Vec<BinaryStatus>, String> {
        ServiceType::all()
            .into_iter()
            .map(|st| self.get_status_sync(st, config_store))
            .collect()
    }

    /// Fetch available versions for a service
    pub async fn get_available_versions(
        &self,
        service_type: ServiceType,
    ) -> Result<Vec<VersionInfo>, String> {
        let service_id = service_type.as_str();
        let registry = ServiceRegistry::load();

        // Try JSON config first
        if let Some(service_config) = registry.get_service(service_id) {
            return self.get_versions_from_config(service_config).await;
        }

        // Fallback to trait-based service
        let service = get_service(service_type);
        let version_source = service.version_source();

        match version_source {
            VersionSource::GitHubReleases(api_url) => {
                self.fetch_github_versions(api_url).await
            }
            VersionSource::Static(versions) => {
                Ok(versions
                    .into_iter()
                    .enumerate()
                    .map(|(i, v)| VersionInfo {
                        version: v.to_string(),
                        is_latest: i == 0,
                        label: None,
                    })
                    .collect())
            }
        }
    }

    /// Fetch versions using JSON config
    async fn get_versions_from_config(
        &self,
        config: &crate::service_config::ServiceConfig,
    ) -> Result<Vec<VersionInfo>, String> {
        let mut versions = match &config.versions {
            VersionConfig::GithubReleases { github_repo } => {
                let api_url = format!(
                    "https://api.github.com/repos/{}/releases",
                    github_repo
                );
                self.fetch_github_versions(&api_url).await?
            }
            VersionConfig::Static { versions } => {
                versions
                    .iter()
                    .enumerate()
                    .map(|(i, v)| VersionInfo {
                        version: v.clone(),
                        is_latest: i == 0,
                        label: None,
                    })
                    .collect()
            }
        };

        // Apply version labels from config
        if !config.version_labels.is_empty() {
            let default_label = config.version_labels.get("default");
            for v in &mut versions {
                v.label = config.version_labels.get(&v.version)
                    .or(default_label)
                    .cloned();
            }
        }

        Ok(versions)
    }

    /// Fetch versions from GitHub releases API
    async fn fetch_github_versions(&self, api_url: &str) -> Result<Vec<VersionInfo>, String> {
        let releases: Vec<GitHubRelease> = self
            .client
            .get(api_url)
            .header("User-Agent", "Burd-App")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch releases: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse releases: {}", e))?;

        let versions: Vec<VersionInfo> = releases
            .into_iter()
            .filter(|r| !r.prerelease)
            .take(20)
            .enumerate()
            .map(|(i, r)| VersionInfo {
                version: r.tag_name,
                is_latest: i == 0,
                label: None,
            })
            .collect();

        if versions.is_empty() {
            Err("No releases found".to_string())
        } else {
            Ok(versions)
        }
    }

    pub async fn download(
        &self,
        service_type: ServiceType,
        version: &str,
        app: AppHandle,
    ) -> Result<BinaryInfo, String> {
        let service_id = service_type.as_str();
        let registry = ServiceRegistry::load();
        let platform = get_current_platform();

        // Create base bin directory
        let bin_dir = get_bin_dir()?;
        fs::create_dir_all(&bin_dir)
            .map_err(|e| format!("Failed to create bin directory: {}", e))?;

        // Try to get download info from JSON config
        let (download_url, final_version, is_archive, binary_name, checksum) =
            if let Some(service_config) = registry.get_service(service_id) {
                let platform_config = service_config
                    .platforms
                    .get(&platform)
                    .ok_or_else(|| format!("No download available for platform: {}", platform))?;

                let binary_name = platform_config
                    .binary_name
                    .clone()
                    .unwrap_or_else(|| service_config.binary_name.clone());

                match &platform_config.download {
                    DownloadConfig::GithubAsset { asset_pattern } => {
                        // Fetch specific release from GitHub
                        if let VersionConfig::GithubReleases { github_repo } = &service_config.versions {
                            let release_url = format!(
                                "https://api.github.com/repos/{}/releases/tags/{}",
                                github_repo, version
                            );
                            let release: GitHubRelease = self
                                .client
                                .get(&release_url)
                                .header("User-Agent", "Burd-App")
                                .send()
                                .await
                                .map_err(|e| format!("Failed to fetch release info: {}", e))?
                                .json()
                                .await
                                .map_err(|e| format!("Failed to parse release info: {}", e))?;

                            let asset = release
                                .assets
                                .iter()
                                .find(|a| a.name == *asset_pattern || a.name.contains(asset_pattern))
                                .ok_or_else(|| {
                                    format!("No binary found (looking for {})", asset_pattern)
                                })?;

                            let final_version = release.tag_name.trim_start_matches('v').to_string();
                            (
                                asset.browser_download_url.clone(),
                                final_version,
                                platform_config.is_archive,
                                binary_name,
                                None, // Checksum not yet supported in JSON config
                            )
                        } else {
                            return Err("GitHub asset download requires GitHub releases version source".to_string());
                        }
                    }
                    DownloadConfig::Direct { url_template, url_template_versioned } => {
                        // Use versioned template if version is not "latest"
                        let template = if version != "latest" {
                            url_template_versioned.as_ref().unwrap_or(url_template)
                        } else {
                            url_template
                        };

                        let clean_version = version.trim_start_matches('v');
                        let url = template
                            .replace("{version}", clean_version)
                            .replace("{VERSION}", version);

                        (url, version.to_string(), platform_config.is_archive, binary_name, None)
                    }
                    DownloadConfig::Homebrew { formula } => {
                        // Handle Homebrew installation - returns early
                        return self.install_homebrew_formula(
                            service_type,
                            formula,
                            &binary_name,
                            &bin_dir,
                            &app,
                        ).await;
                    }
                    DownloadConfig::Npm { package } => {
                        // NPM packages are installed per-instance, not downloaded as binaries
                        // Return a "virtual" BinaryInfo that marks this version as available
                        return Ok(BinaryInfo {
                            version: version.to_string(),
                            path: format!("npm:{}", package),
                            downloaded_at: chrono::Utc::now(),
                        });
                    }
                }
            } else {
                // Fallback to trait-based service
                let service = get_service(service_type);
                let arch = if cfg!(target_arch = "aarch64") { "aarch64" } else { "x86_64" };
                let download_method = service.download_method(version, arch);

                match download_method {
                    DownloadMethod::GitHubRelease { api_url, asset_pattern, checksum } => {
                        let release_url = format!("{}{}", api_url, version);
                        let release: GitHubRelease = self
                            .client
                            .get(&release_url)
                            .header("User-Agent", "Burd-App")
                            .send()
                            .await
                            .map_err(|e| format!("Failed to fetch release info: {}", e))?
                            .json()
                            .await
                            .map_err(|e| format!("Failed to parse release info: {}", e))?;

                        let asset = release
                            .assets
                            .iter()
                            .find(|a| a.name == asset_pattern || a.name.contains(&asset_pattern))
                            .ok_or_else(|| {
                                format!(
                                    "No binary found for {} (looking for {})",
                                    service.display_name(),
                                    asset_pattern
                                )
                            })?;

                        let final_version = release.tag_name.trim_start_matches('v').to_string();
                        (asset.browser_download_url.clone(), final_version, false, service.binary_name().to_string(), checksum)
                    }
                    DownloadMethod::Direct { url, is_archive, checksum } => {
                        (url, version.to_string(), is_archive, service.binary_name().to_string(), checksum)
                    }
                }
            };

        // Download with progress
        let response = self
            .client
            .get(&download_url)
            .header("User-Agent", "Burd-App")
            .send()
            .await
            .map_err(|e| format!("Failed to download binary: {}", e))?;

        if !response.status().is_success() {
            return Err(format!(
                "Download failed with status {}: {}",
                response.status(),
                download_url
            ));
        }

        let total_size = response.content_length().unwrap_or(0);

        // Migrate legacy flat binary if it exists
        // Old structure: bin/meilisearch (a file)
        // New structure: bin/meilisearch/1.6.0/meilisearch (directory with version)
        let legacy_binary_path = get_binary_path(service_type)?;
        let service_dir = get_service_bin_dir(service_type)?;

        if legacy_binary_path.exists() && legacy_binary_path.is_file() {
            // There's a legacy flat binary - we need to migrate it
            // 1. Create a temp file name
            let temp_path = bin_dir.join(format!("{}_legacy_temp", service_type.as_str()));
            // 2. Move legacy binary to temp location
            fs::rename(&legacy_binary_path, &temp_path)
                .map_err(|e| format!("Failed to move legacy binary for migration: {}", e))?;
            // 3. Create the service directory
            fs::create_dir_all(&service_dir)
                .map_err(|e| format!("Failed to create service directory: {}", e))?;
            // 4. Create a "legacy" version directory and move the binary there
            let legacy_version_dir = service_dir.join("legacy");
            fs::create_dir_all(&legacy_version_dir)
                .map_err(|e| format!("Failed to create legacy version directory: {}", e))?;
            let legacy_dest = legacy_version_dir.join(&binary_name);
            fs::rename(&temp_path, &legacy_dest)
                .map_err(|e| format!("Failed to move legacy binary to version directory: {}", e))?;
        }

        // Create versioned directory: bin/{service_type}/{version}/
        let version_dir = get_versioned_binary_dir(service_type, &final_version)?;
        fs::create_dir_all(&version_dir)
            .map_err(|e| format!("Failed to create version directory: {}", e))?;

        let binary_path = version_dir.join(&binary_name);

        // For archives, download to temp file first
        let download_path = if is_archive {
            // Determine extension from URL
            let ext = if download_url.ends_with(".zip") {
                "zip"
            } else {
                "tar.gz"
            };
            version_dir.join(format!("{}.{}", binary_name, ext))
        } else {
            binary_path.clone()
        };

        let mut file = File::create(&download_path)
            .map_err(|e| format!("Failed to create download file: {}", e))?;

        let mut downloaded: u64 = 0;
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
            file.write_all(&chunk)
                .map_err(|e| format!("Failed to write chunk: {}", e))?;

            downloaded += chunk.len() as u64;

            let progress = DownloadProgress {
                service_type: service_type.as_str().to_string(),
                downloaded,
                total: total_size,
                percentage: if total_size > 0 {
                    (downloaded as f64 / total_size as f64) * 100.0
                } else {
                    0.0
                },
                phase: "downloading".to_string(),
            };

            let _ = app.emit("download-progress", progress);
        }

        drop(file); // Close file before extraction

        // Verify checksum if provided
        if let Some(expected_checksum) = checksum {
            verify_checksum(&download_path, expected_checksum)?;
        }

        // Emit extracting phase
        if is_archive {
            let _ = app.emit("download-progress", DownloadProgress {
                service_type: service_type.as_str().to_string(),
                downloaded: total_size,
                total: total_size,
                percentage: 100.0,
                phase: "extracting".to_string(),
            });
        }

        // Extract archive if needed
        if is_archive {
            // Check if it's a zip file or tar.gz
            let is_zip = download_url.ends_with(".zip")
                || download_path.extension().is_some_and(|ext| ext == "zip");

            if is_zip {
                extract_zip(&download_path, &version_dir)?;
            } else {
                extract_tar_gz(&download_path, &version_dir, &binary_name)?;
            }
            // Remove the archive after extraction
            let _ = fs::remove_file(&download_path);
        }

        // Make executable and remove quarantine attribute (macOS Gatekeeper)
        make_binary_executable(&binary_path)?;

        // Return binary info
        let binary_info = BinaryInfo {
            version: final_version,
            path: binary_path.to_string_lossy().to_string(),
            downloaded_at: Utc::now(),
        };

        Ok(binary_info)
    }

    /// Install a service via Homebrew and create symlink
    async fn install_homebrew_formula(
        &self,
        service_type: ServiceType,
        formula: &str,
        binary_name: &str,
        bin_dir: &std::path::Path,
        app: &AppHandle,
    ) -> Result<BinaryInfo, String> {
        use std::process::Command;

        // Emit initial progress
        let _ = app.emit("download-progress", DownloadProgress {
            service_type: service_type.as_str().to_string(),
            downloaded: 0,
            total: 0,
            percentage: -1.0, // Indeterminate
            phase: "installing".to_string(),
        });

        // Check if Homebrew is available
        let brew_check = Command::new("brew")
            .arg("--version")
            .output()
            .map_err(|_| "Homebrew is not installed. Install from https://brew.sh")?;

        if !brew_check.status.success() {
            return Err("Homebrew is not installed. Install from https://brew.sh".to_string());
        }

        // Check if formula is already installed
        let list_output = Command::new("brew")
            .args(["list", formula])
            .output()
            .map_err(|e| format!("Failed to check brew: {}", e))?;

        if !list_output.status.success() {
            // Install the formula
            let _ = app.emit("download-progress", DownloadProgress {
                service_type: service_type.as_str().to_string(),
                downloaded: 0,
                total: 0,
                percentage: -1.0,
                phase: "installing".to_string(),
            });

            let install_output = Command::new("brew")
                .args(["install", formula])
                .output()
                .map_err(|e| format!("Failed to install {}: {}", formula, e))?;

            if !install_output.status.success() {
                return Err(format!(
                    "brew install failed: {}",
                    String::from_utf8_lossy(&install_output.stderr)
                ));
            }
        }

        // Get the prefix path (e.g., /opt/homebrew/opt/mariadb)
        let prefix_output = Command::new("brew")
            .args(["--prefix", formula])
            .output()
            .map_err(|e| format!("Failed to get prefix: {}", e))?;

        if !prefix_output.status.success() {
            return Err(format!("Formula '{}' not found", formula));
        }

        let prefix = String::from_utf8_lossy(&prefix_output.stdout)
            .trim()
            .to_string();

        // Get the Cellar path to find the version
        let cellar_output = Command::new("brew")
            .args(["--cellar", formula])
            .output()
            .map_err(|e| format!("Failed to get cellar: {}", e))?;

        let cellar = String::from_utf8_lossy(&cellar_output.stdout)
            .trim()
            .to_string();

        // Find version directory (get the latest/newest one)
        let version = fs::read_dir(&cellar)
            .map_err(|e| format!("Failed to read Cellar at {}: {}", cellar, e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .max_by_key(|e| e.file_name())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .ok_or_else(|| format!("No version found in Cellar: {}", cellar))?;

        // Find the binary in prefix/bin/
        let source_binary = std::path::PathBuf::from(&prefix)
            .join("bin")
            .join(binary_name);

        if !source_binary.exists() {
            return Err(format!(
                "Binary '{}' not found at {:?}. Available in {}/bin/: {:?}",
                binary_name,
                source_binary,
                prefix,
                fs::read_dir(format!("{}/bin", prefix))
                    .map(|entries| entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect::<Vec<_>>())
                    .unwrap_or_default()
            ));
        }

        // Migrate legacy flat binary/symlink if it exists
        let legacy_binary_path = get_binary_path(service_type)?;
        let service_dir = get_service_bin_dir(service_type)?;

        if legacy_binary_path.exists() && (legacy_binary_path.is_file() || legacy_binary_path.is_symlink()) {
            // There's a legacy flat binary - we need to migrate it
            let temp_path = bin_dir.join(format!("{}_legacy_temp", service_type.as_str()));
            fs::rename(&legacy_binary_path, &temp_path)
                .map_err(|e| format!("Failed to move legacy binary for migration: {}", e))?;
            fs::create_dir_all(&service_dir)
                .map_err(|e| format!("Failed to create service directory: {}", e))?;
            let legacy_version_dir = service_dir.join("legacy");
            fs::create_dir_all(&legacy_version_dir)
                .map_err(|e| format!("Failed to create legacy version directory: {}", e))?;
            let legacy_dest = legacy_version_dir.join(binary_name);
            fs::rename(&temp_path, &legacy_dest)
                .map_err(|e| format!("Failed to move legacy binary to version directory: {}", e))?;
        }

        // Create versioned directory: bin/{service_type}/{version}/
        let version_dir = get_versioned_binary_dir(service_type, &version)?;
        fs::create_dir_all(&version_dir)
            .map_err(|e| format!("Failed to create version directory: {}", e))?;

        let dest_binary = version_dir.join(binary_name);

        // Remove existing symlink/file if exists
        let _ = fs::remove_file(&dest_binary);

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&source_binary, &dest_binary)
                .map_err(|e| format!("Failed to create symlink: {}", e))?;
        }

        // Emit completion
        let _ = app.emit("download-progress", DownloadProgress {
            service_type: service_type.as_str().to_string(),
            downloaded: 100,
            total: 100,
            percentage: 100.0,
            phase: "complete".to_string(),
        });

        Ok(BinaryInfo {
            version,
            path: dest_binary.to_string_lossy().to_string(),
            downloaded_at: Utc::now(),
        })
    }

    /// Delete a specific version of a binary
    pub fn delete_version(&self, service_type: ServiceType, version: &str) -> Result<(), String> {
        let version_dir = get_versioned_binary_dir(service_type, version)?;

        if version_dir.exists() {
            fs::remove_dir_all(&version_dir)
                .map_err(|e| format!("Failed to delete version directory: {}", e))?;
        }

        Ok(())
    }
}

/// Extract a zip archive using system unzip command
fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<(), String> {
    use std::process::Command;

    // Use system unzip command
    let output = Command::new("unzip")
        .args([
            "-o",  // overwrite without prompting
            "-q",  // quiet
            archive_path.to_str().ok_or("Invalid archive path")?,
            "-d",
            dest_dir.to_str().ok_or("Invalid destination path")?,
        ])
        .output()
        .map_err(|e| format!("Failed to run unzip: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to extract zip: {}", stderr));
    }

    Ok(())
}

/// Extract a tar.gz archive and find the binary
fn extract_tar_gz(archive_path: &Path, dest_dir: &Path, binary_name: &str) -> Result<(), String> {
    let file = File::open(archive_path)
        .map_err(|e| format!("Failed to open archive: {}", e))?;

    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    // Create a temp extraction directory
    let extract_dir = dest_dir.join("_extract_temp");
    fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    archive
        .unpack(&extract_dir)
        .map_err(|e| format!("Failed to extract archive: {}", e))?;

    // Check if this is a bundled package (has lib/ folder with dylibs)
    // If so, preserve the entire structure
    let has_bundled_libs = find_lib_dir(&extract_dir).is_some();

    if has_bundled_libs {
        // This is a bundled package - preserve entire structure
        copy_bundled_package(&extract_dir, dest_dir, binary_name)?;
    } else {
        // Legacy behavior - just find and move the binary
        let binary_dest = dest_dir.join(binary_name);
        let found = find_and_move_binary(&extract_dir, binary_name, &binary_dest)?;
        if !found {
            // Clean up temp directory
            let _ = fs::remove_dir_all(&extract_dir);
            return Err(format!(
                "Binary '{}' not found in archive",
                binary_name
            ));
        }
    }

    // Clean up temp directory
    let _ = fs::remove_dir_all(&extract_dir);

    Ok(())
}

/// Find a lib directory containing dylibs in the extracted archive
fn find_lib_dir(dir: &Path) -> Option<PathBuf> {
    // Check direct lib/ folder
    let lib_dir = dir.join("lib");
    if lib_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&lib_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".dylib") || name.ends_with(".so") {
                        return Some(lib_dir);
                    }
                }
            }
        }
    }

    // Check one level deep (e.g., redis/8.4.0/lib/)
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_lib_dir(&path) {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// Patch PostgreSQL binaries to use correct share and lib paths
/// Homebrew bottles have hardcoded paths like /opt/homebrew/share/postgresql@17
/// We replace these with the actual installation paths
fn patch_postgresql_paths(bin_dir: &Path, share_path: &str, lib_path: &str) -> Result<(), String> {
    use std::process::Command;

    // Paths to patch (must be same length or shorter + null padding)
    // Original: /opt/homebrew/share/postgresql@17 (33 chars)
    // Original: /opt/homebrew/lib/postgresql@17 (31 chars)
    let old_share = "/opt/homebrew/share/postgresql@17";
    let old_lib = "/opt/homebrew/lib/postgresql@17";

    // Pad new paths with null bytes to match original length
    let new_share = format!("{}\0", share_path);
    let new_lib = format!("{}\0", lib_path);

    // Check if new paths would fit (they need to be <= original length)
    if new_share.len() > old_share.len() + 1 {
        return Err(format!(
            "Share path too long: {} chars (max {})",
            share_path.len(),
            old_share.len()
        ));
    }
    if new_lib.len() > old_lib.len() + 1 {
        return Err(format!(
            "Lib path too long: {} chars (max {})",
            lib_path.len(),
            old_lib.len()
        ));
    }

    // Pad to exact length with null bytes
    let padded_share = format!(
        "{}{}",
        share_path,
        "\0".repeat(old_share.len() - share_path.len())
    );
    let padded_lib = format!(
        "{}{}",
        lib_path,
        "\0".repeat(old_lib.len() - lib_path.len())
    );

    // Process each binary in the bin directory
    if let Ok(entries) = fs::read_dir(bin_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            // Read binary content
            let content = match fs::read(&path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            // Check if binary contains paths to patch
            let has_share = content
                .windows(old_share.len())
                .any(|w| w == old_share.as_bytes());
            let has_lib = content
                .windows(old_lib.len())
                .any(|w| w == old_lib.as_bytes());

            if !has_share && !has_lib {
                continue;
            }

            // Patch the binary
            let mut patched = content.clone();

            // Replace share path
            if has_share {
                let old_bytes = old_share.as_bytes();
                let new_bytes = padded_share.as_bytes();
                let mut i = 0;
                while i <= patched.len() - old_bytes.len() {
                    if &patched[i..i + old_bytes.len()] == old_bytes {
                        patched[i..i + new_bytes.len()].copy_from_slice(new_bytes);
                        i += new_bytes.len();
                    } else {
                        i += 1;
                    }
                }
            }

            // Replace lib path
            if has_lib {
                let old_bytes = old_lib.as_bytes();
                let new_bytes = padded_lib.as_bytes();
                let mut i = 0;
                while i <= patched.len() - old_bytes.len() {
                    if &patched[i..i + old_bytes.len()] == old_bytes {
                        patched[i..i + new_bytes.len()].copy_from_slice(new_bytes);
                        i += new_bytes.len();
                    } else {
                        i += 1;
                    }
                }
            }

            // Write patched binary
            if let Err(e) = fs::write(&path, &patched) {
                eprintln!("Failed to patch {}: {}", path.display(), e);
                continue;
            }

            // Re-sign after patching (required on Apple Silicon)
            let _ = Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(&path)
                .output();
        }
    }

    Ok(())
}

/// Copy a bundled package preserving lib/ structure
fn copy_bundled_package(extract_dir: &Path, dest_dir: &Path, binary_name: &str) -> Result<(), String> {
    use std::process::Command;

    // Find the root of the package (where bin/ and lib/ are)
    let package_root = find_package_root(extract_dir, binary_name)
        .ok_or_else(|| format!("Could not find package root with binary '{}'", binary_name))?;

    // Copy lib/ folder if it exists
    let src_lib = package_root.join("lib");
    let has_lib = src_lib.is_dir();
    if has_lib {
        let dest_lib = dest_dir.join("lib");
        fs::create_dir_all(&dest_lib)
            .map_err(|e| format!("Failed to create lib directory: {}", e))?;

        copy_dir_contents(&src_lib, &dest_lib)?;

        // Make all dylibs executable
        if let Ok(entries) = fs::read_dir(&dest_lib) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = make_binary_executable(&path);
                }
            }
        }
    }

    // Copy share/ folder if it exists (needed for PostgreSQL, etc.)
    let src_share = package_root.join("share");
    if src_share.is_dir() {
        let dest_share = dest_dir.join("share");
        fs::create_dir_all(&dest_share)
            .map_err(|e| format!("Failed to create share directory: {}", e))?;
        copy_dir_contents(&src_share, &dest_share)?;
    }

    // Copy bin/ folder (for additional binaries like psql, pg_dump, etc.)
    let src_bin = package_root.join("bin");
    if src_bin.is_dir() {
        let dest_bin = dest_dir.join("bin");
        fs::create_dir_all(&dest_bin)
            .map_err(|e| format!("Failed to create bin directory: {}", e))?;
        copy_dir_contents(&src_bin, &dest_bin)?;

        // Make all binaries executable
        if let Ok(entries) = fs::read_dir(&dest_bin) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let _ = make_binary_executable(&path);
                }
            }
        }

        // For PostgreSQL, patch binaries to use /opt/burd/share and /opt/burd/lib
        // Then create symlinks from /opt/burd/* to the actual directories
        let dest_share = dest_dir.join("share");
        let dest_lib_dir = dest_dir.join("lib");
        if dest_share.exists() && dest_lib_dir.exists() {
            // Check if this looks like PostgreSQL (has timezonesets)
            if dest_share.join("timezonesets").exists() {
                // Patch binaries to use /opt/burd paths
                if let Err(e) = patch_postgresql_paths(&dest_bin, "/opt/burd/share", "/opt/burd/lib") {
                    eprintln!("Warning: Failed to patch PostgreSQL paths: {}", e);
                }

                // Create symlinks in /opt/burd/
                // Note: /opt/burd must exist and be owned by the user (setup via helper)
                let opt_burd = Path::new("/opt/burd");
                if opt_burd.exists() {
                    let share_link = opt_burd.join("share");
                    let lib_link = opt_burd.join("lib");

                    // Remove existing symlinks and create new ones
                    let _ = fs::remove_file(&share_link);
                    let _ = fs::remove_file(&lib_link);

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::symlink;
                        if let Err(e) = symlink(&dest_share, &share_link) {
                            eprintln!("Warning: Failed to create /opt/burd/share symlink: {}", e);
                        }
                        if let Err(e) = symlink(&dest_lib_dir, &lib_link) {
                            eprintln!("Warning: Failed to create /opt/burd/lib symlink: {}", e);
                        }
                    }
                }
            }
        }
    }

    // Find and copy the main binary to version root (for backwards compatibility)
    // For PostgreSQL, copy from dest_bin (which has patched binaries)
    // For others, copy from original source
    let binary_dest = dest_dir.join(binary_name);
    let dest_bin = dest_dir.join("bin");
    let patched_main_binary = dest_bin.join(binary_name);

    if patched_main_binary.exists() {
        // Copy from patched bin/ directory (for PostgreSQL and similar)
        fs::copy(&patched_main_binary, &binary_dest)
            .map_err(|e| format!("Failed to copy binary: {}", e))?;
        make_binary_executable(&binary_dest)?;
    } else {
        // Fallback: copy from original source
        let main_binary_src = package_root.join("bin").join(binary_name);
        if main_binary_src.exists() {
            fs::copy(&main_binary_src, &binary_dest)
                .map_err(|e| format!("Failed to copy binary: {}", e))?;
            make_binary_executable(&binary_dest)?;
        } else {
            // Try to find binary directly in package root
            let src_bin_direct = package_root.join(binary_name);
            if src_bin_direct.exists() {
                fs::copy(&src_bin_direct, &binary_dest)
                    .map_err(|e| format!("Failed to copy binary: {}", e))?;
                make_binary_executable(&binary_dest)?;
            } else {
                return Err(format!("Binary '{}' not found in package", binary_name));
            }
        }
    }

    // Fix dylib paths: change @executable_path/../lib/ to @executable_path/lib/
    // This is needed because we place the binary directly in version_dir/ instead of version_dir/bin/
    if has_lib {
        // Get list of dylibs to fix
        let dest_lib = dest_dir.join("lib");
        if let Ok(entries) = fs::read_dir(&dest_lib) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.ends_with(".dylib") {
                        let old_path = format!("@executable_path/../lib/{}", name);
                        let new_path = format!("@executable_path/lib/{}", name);

                        // Fix path in the main binary
                        let _ = Command::new("install_name_tool")
                            .arg("-change")
                            .arg(&old_path)
                            .arg(&new_path)
                            .arg(&binary_dest)
                            .output();

                        // Also fix the install name of the dylib itself
                        let _ = Command::new("install_name_tool")
                            .arg("-id")
                            .arg(&new_path)
                            .arg(&path)
                            .output();
                    }
                }
            }
        }

        // Note: Inter-dylib references use @loader_path/ which is already correct
        // from our bottle-extractor, so we skip the expensive nested loop.

        // Re-sign everything after modification (required on Apple Silicon)
        let _ = Command::new("codesign")
            .args(["--force", "--sign", "-"])
            .arg(&binary_dest)
            .output();

        if let Ok(entries) = fs::read_dir(&dest_lib) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("dylib") {
                    let _ = Command::new("codesign")
                        .args(["--force", "--sign", "-"])
                        .arg(&path)
                        .output();
                }
            }
        }
    }

    Ok(())
}

/// Find the root directory of a package (where bin/ or the binary is located)
fn find_package_root(dir: &Path, binary_name: &str) -> Option<PathBuf> {
    // Check if bin/binary_name exists here
    if dir.join("bin").join(binary_name).exists() {
        return Some(dir.to_path_buf());
    }

    // Check if binary exists directly here
    if dir.join(binary_name).exists() {
        return Some(dir.to_path_buf());
    }

    // Search subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_package_root(&path, binary_name) {
                    return Some(found);
                }
            }
        }
    }

    None
}

/// Copy contents of a directory recursively
fn copy_dir_contents(src: &Path, dest: &Path) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(src) {
        for entry in entries.flatten() {
            let src_path = entry.path();
            let Some(file_name) = src_path.file_name() else {
                continue; // Skip entries with no file name
            };
            let dest_path = dest.join(file_name);

            // Check if source is a symlink
            let metadata = fs::symlink_metadata(&src_path)
                .map_err(|e| format!("Failed to get metadata for {}: {}", src_path.display(), e))?;

            if metadata.is_symlink() {
                // Handle symlinks: read the link target and recreate it
                let link_target = fs::read_link(&src_path)
                    .map_err(|e| format!("Failed to read symlink {}: {}", src_path.display(), e))?;

                // Remove existing file/symlink if it exists
                let _ = fs::remove_file(&dest_path);

                #[cfg(unix)]
                std::os::unix::fs::symlink(&link_target, &dest_path)
                    .map_err(|e| format!("Failed to create symlink {}: {}", dest_path.display(), e))?;
            } else if metadata.is_dir() {
                fs::create_dir_all(&dest_path)
                    .map_err(|e| format!("Failed to create directory: {}", e))?;
                copy_dir_contents(&src_path, &dest_path)?;
            } else {
                // Remove existing file if it exists (handles permission issues)
                let _ = fs::remove_file(&dest_path);

                fs::copy(&src_path, &dest_path)
                    .map_err(|e| format!("Failed to copy file {}: {}", src_path.display(), e))?;
            }
        }
    }
    Ok(())
}

/// Recursively find a binary in extracted directory and move it to destination
fn find_and_move_binary(dir: &Path, binary_name: &str, dest: &Path) -> Result<bool, String> {
    if !dir.is_dir() {
        return Ok(false);
    }

    for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))? {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_file() {
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if file_name == binary_name {
                fs::rename(&path, dest)
                    .or_else(|_| {
                        // If rename fails (cross-device), try copy + delete
                        fs::copy(&path, dest)?;
                        fs::remove_file(&path)
                    })
                    .map_err(|e| format!("Failed to move binary: {}", e))?;
                // Make executable after moving
                make_binary_executable(dest)?;
                return Ok(true);
            }
        } else if path.is_dir() && find_and_move_binary(&path, binary_name, dest)? {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Make a binary executable and remove macOS quarantine attribute
/// This avoids Gatekeeper prompts and permission issues
fn make_binary_executable(binary_path: &Path) -> Result<(), String> {
    use std::process::Command;

    // Set executable permissions (chmod 755)
    let mut perms = fs::metadata(binary_path)
        .map_err(|e| format!("Failed to get file metadata: {}", e))?
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(binary_path, perms)
        .map_err(|e| format!("Failed to set executable permission: {}", e))?;

    // Remove macOS quarantine attribute (xattr -cr)
    // This prevents Gatekeeper from blocking the binary
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("xattr")
            .args(["-cr", &binary_path.to_string_lossy()])
            .output();
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_verify_checksum_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        // Write test data
        let test_data = b"Hello, World!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data).unwrap();
        drop(file);

        // Compute expected checksum (SHA256 of "Hello, World!")
        let expected_checksum = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";

        // Verify checksum - should succeed
        let result = verify_checksum(&file_path, expected_checksum);
        assert!(result.is_ok(), "Checksum verification should succeed");
    }

    #[test]
    fn test_verify_checksum_failure() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        // Write test data
        let test_data = b"Hello, World!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data).unwrap();
        drop(file);

        // Use wrong checksum
        let wrong_checksum = "0000000000000000000000000000000000000000000000000000000000000000";

        // Verify checksum - should fail
        let result = verify_checksum(&file_path, wrong_checksum);
        assert!(result.is_err(), "Checksum verification should fail with wrong checksum");
        assert!(result.unwrap_err().contains("Checksum verification failed"));
    }

    #[test]
    fn test_verify_checksum_case_insensitive() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.bin");

        // Write test data
        let test_data = b"Hello, World!";
        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data).unwrap();
        drop(file);

        // Uppercase checksum
        let uppercase_checksum = "DFFD6021BB2BD5B0AF676290809EC3A53191DD81C7F70A4B28688A362182986F";

        // Should still verify successfully (case-insensitive)
        let result = verify_checksum(&file_path, uppercase_checksum);
        assert!(result.is_ok(), "Checksum verification should be case-insensitive");
    }

    #[test]
    fn test_verify_checksum_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.bin");

        let checksum = "0000000000000000000000000000000000000000000000000000000000000000";

        // Should fail with file not found error
        let result = verify_checksum(&file_path, checksum);
        assert!(result.is_err(), "Should fail for nonexistent file");
        assert!(result.unwrap_err().contains("Failed to open file"));
    }

    #[test]
    fn test_verify_checksum_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.bin");

        // Create empty file
        File::create(&file_path).unwrap();

        // SHA256 of empty file
        let empty_checksum = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

        // Should verify successfully
        let result = verify_checksum(&file_path, empty_checksum);
        assert!(result.is_ok(), "Should verify empty file successfully");
    }

    #[test]
    fn test_verify_checksum_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.bin");

        // Create a large file (1MB of zeros)
        let mut file = File::create(&file_path).unwrap();
        let chunk = vec![0u8; 8192];
        for _ in 0..128 {
            file.write_all(&chunk).unwrap();
        }
        drop(file);

        // SHA256 of 1MB of zeros
        let expected_checksum = "30e14955ebf1352266dc2ff8067e68104607e750abb9d3b36582b8af909fcb58";

        // Should verify successfully
        let result = verify_checksum(&file_path, expected_checksum);
        assert!(result.is_ok(), "Should verify large file successfully");
    }
}
