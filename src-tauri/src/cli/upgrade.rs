//! Upgrade CLI command
//!
//! Self-update the burd CLI to the latest version.

use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::io::{self, Write};

/// Update manifest URL
/// Options:
/// - Custom domain: https://updates.burd.dev/cli/latest.json
/// - Scaleway direct: https://burd-updates.s3.fr-par.scw.cloud/cli/latest.json
const UPDATE_URL: &str = "https://burd-updates.s3.fr-par.scw.cloud/cli/latest.json";

/// Fallback to GitHub releases API
/// TODO: Update with your actual GitHub repository
const GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/yourusername/burd/releases/latest";

/// Update manifest structure
#[derive(serde::Deserialize, Debug)]
struct UpdateManifest {
    version: String,
    notes: Option<String>,
    #[allow(dead_code)]
    pub_date: Option<String>,
    platforms: std::collections::HashMap<String, PlatformInfo>,
}

#[derive(serde::Deserialize, Debug)]
struct PlatformInfo {
    url: String,
    sha256: String,
}

/// Run the upgrade command
///
/// Checks for updates and installs if available.
pub fn run_upgrade(check_only: bool) -> Result<(), String> {
    let current_version = env!("CARGO_PKG_VERSION");

    println!("Burd CLI v{}", current_version);
    println!();
    println!("Checking for updates...");

    // Try to fetch update manifest
    let manifest = fetch_update_manifest()?;

    // Compare versions
    if !is_newer_version(&manifest.version, current_version) {
        println!();
        println!("You're already on the latest version.");
        return Ok(());
    }

    println!();
    println!(
        "New version available: {} -> {}",
        current_version, manifest.version
    );

    if let Some(notes) = &manifest.notes {
        println!();
        println!("Release notes:");
        for line in notes.lines() {
            println!("  {}", line);
        }
    }

    if check_only {
        println!();
        println!("Run 'burd upgrade' to install the update.");
        return Ok(());
    }

    println!();
    print!("Install update? [Y/n] ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| format!("Failed to read input: {}", e))?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("Update cancelled.");
        return Ok(());
    }

    // Get platform info
    let platform_key = get_platform_key();
    let platform_info = manifest.platforms.get(&platform_key).ok_or_else(|| {
        format!(
            "No update available for your platform: {}\n\
             Available platforms: {:?}",
            platform_key,
            manifest.platforms.keys().collect::<Vec<_>>()
        )
    })?;

    // Download the update
    println!();
    println!("Downloading from {}...", platform_info.url);

    let client = reqwest::blocking::Client::builder()
        .user_agent("burd-cli")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&platform_info.url)
        .send()
        .map_err(|e| format!("Failed to download update: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download update: HTTP {}",
            response.status()
        ));
    }

    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read download: {}", e))?;

    println!("Downloaded {} bytes", bytes.len());

    // Verify checksum
    println!("Verifying checksum...");

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let hash = format!("{:x}", hasher.finalize());

    if hash != platform_info.sha256 {
        return Err(format!(
            "Checksum verification failed!\n\
             Expected: {}\n\
             Got: {}\n\n\
             The download may be corrupted. Please try again.",
            platform_info.sha256, hash
        ));
    }

    println!("Checksum verified.");

    // Replace the binary
    println!("Installing update...");

    let current_exe =
        env::current_exe().map_err(|e| format!("Failed to get current executable path: {}", e))?;

    let backup_path = current_exe.with_extension("old");
    let temp_path = current_exe.with_extension("new");

    // Write new binary to temp location first
    fs::write(&temp_path, &bytes).map_err(|e| format!("Failed to write new binary: {}", e))?;

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Backup current binary
    if let Err(e) = fs::rename(&current_exe, &backup_path) {
        // Clean up temp file
        let _ = fs::remove_file(&temp_path);
        return Err(format!("Failed to backup current binary: {}", e));
    }

    // Move new binary into place
    if let Err(e) = fs::rename(&temp_path, &current_exe) {
        // Restore backup on failure
        let _ = fs::rename(&backup_path, &current_exe);
        return Err(format!("Failed to install new binary: {}", e));
    }

    // Remove backup
    let _ = fs::remove_file(&backup_path);

    println!();
    println!("Successfully upgraded to version {}!", manifest.version);
    println!();
    println!("Run 'burd --version' to verify.");

    Ok(())
}

/// Fetch the update manifest from the server
fn fetch_update_manifest() -> Result<UpdateManifest, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("burd-cli")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Try primary update URL first
    match client.get(UPDATE_URL).send() {
        Ok(response) if response.status().is_success() => {
            return response
                .json::<UpdateManifest>()
                .map_err(|e| format!("Failed to parse update manifest: {}", e));
        }
        Ok(response) => {
            eprintln!(
                "Warning: Update server returned HTTP {}, trying fallback...",
                response.status()
            );
        }
        Err(e) => {
            eprintln!("Warning: Could not reach update server: {}", e);
            eprintln!("Trying GitHub releases...");
        }
    }

    // Fallback to GitHub releases
    fetch_from_github(&client)
}

/// Fetch update info from GitHub releases API
fn fetch_from_github(client: &reqwest::blocking::Client) -> Result<UpdateManifest, String> {
    let response = client
        .get(GITHUB_RELEASES_URL)
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .map_err(|e| format!("Failed to fetch from GitHub: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "GitHub API returned HTTP {}. \n\
             Check your internet connection or try again later.",
            response.status()
        ));
    }

    #[derive(serde::Deserialize)]
    struct GitHubRelease {
        tag_name: String,
        body: Option<String>,
        published_at: Option<String>,
        assets: Vec<GitHubAsset>,
    }

    #[derive(serde::Deserialize)]
    struct GitHubAsset {
        name: String,
        browser_download_url: String,
    }

    let release: GitHubRelease = response
        .json()
        .map_err(|e| format!("Failed to parse GitHub response: {}", e))?;

    // Parse version from tag (remove 'v' prefix)
    let version = release.tag_name.trim_start_matches('v').to_string();

    // Find CLI binaries in assets
    let mut platforms = std::collections::HashMap::new();

    for asset in &release.assets {
        if asset.name.starts_with("burd-darwin-aarch64") && !asset.name.ends_with(".sha256") {
            // Try to find the sha256 file
            let sha256 = release
                .assets
                .iter()
                .find(|a| a.name == format!("{}.sha256", asset.name))
                .map(|a| fetch_sha256(client, &a.browser_download_url))
                .transpose()?
                .unwrap_or_default();

            platforms.insert(
                "darwin-aarch64".to_string(),
                PlatformInfo {
                    url: asset.browser_download_url.clone(),
                    sha256,
                },
            );
        } else if asset.name.starts_with("burd-darwin-x64") && !asset.name.ends_with(".sha256") {
            let sha256 = release
                .assets
                .iter()
                .find(|a| a.name == format!("{}.sha256", asset.name))
                .map(|a| fetch_sha256(client, &a.browser_download_url))
                .transpose()?
                .unwrap_or_default();

            platforms.insert(
                "darwin-x64".to_string(),
                PlatformInfo {
                    url: asset.browser_download_url.clone(),
                    sha256,
                },
            );
        }
    }

    if platforms.is_empty() {
        return Err(
            "No CLI binaries found in the latest release.\n\
             Please check https://github.com/yourusername/burd/releases"
                .to_string(),
        );
    }

    Ok(UpdateManifest {
        version,
        notes: release.body,
        pub_date: release.published_at,
        platforms,
    })
}

/// Fetch SHA256 checksum from URL
fn fetch_sha256(client: &reqwest::blocking::Client, url: &str) -> Result<String, String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("Failed to fetch checksum: {}", e))?;

    if !response.status().is_success() {
        return Ok(String::new()); // Checksum optional
    }

    let text = response
        .text()
        .map_err(|e| format!("Failed to read checksum: {}", e))?;

    // SHA256 file might be just the hash or "hash  filename"
    Ok(text.split_whitespace().next().unwrap_or("").to_string())
}

/// Get platform key for current system
fn get_platform_key() -> String {
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        "unknown"
    };

    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    format!("{}-{}", os, arch)
}

/// Compare version strings (simple semver comparison)
fn is_newer_version(new: &str, current: &str) -> bool {
    let parse_version = |v: &str| -> (u32, u32, u32) {
        let parts: Vec<&str> = v.split('-').next().unwrap_or(v).split('.').collect();
        (
            parts.first().and_then(|s| s.parse().ok()).unwrap_or(0),
            parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
            parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0),
        )
    };

    let new_v = parse_version(new);
    let current_v = parse_version(current);

    new_v > current_v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        assert!(is_newer_version("1.0.1", "1.0.0"));
        assert!(is_newer_version("1.1.0", "1.0.0"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.1"));
        assert!(!is_newer_version("0.9.0", "1.0.0"));
    }

    #[test]
    fn test_platform_key() {
        let key = get_platform_key();
        assert!(key.contains("darwin") || key.contains("linux") || key.contains("windows"));
        assert!(key.contains("aarch64") || key.contains("x64"));
    }
}
