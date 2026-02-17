//! PHP Version Manager (PVM)
//!
//! Manages PHP CLI versions by downloading static binaries from static-php.dev.
//! Similar to NVM but for PHP, with shell integration for version switching.

use crate::config::get_app_dir;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tauri::{AppHandle, Emitter};

/// Base URL for PHP binary downloads
const DOWNLOAD_BASE_URL: &str = "https://dl.static-php.dev/static-php-cli/common";

/// Shell profile marker comment
const SHELL_MARKER: &str = "# Added by Burd - PHP Version Manager";

/// Information about a PHP version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PHPVersion {
    pub version: String,
    pub is_default: bool,
}

/// Status of the PHP version manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PvmStatus {
    pub pvm_dir: String,
    pub default_version: Option<String>,
    pub installed_count: usize,
    pub current_php: Option<CurrentPHP>,
    pub burd_php: Option<CurrentPHP>,
    pub shell_configured: bool,
}

/// Information about the currently active PHP in PATH
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentPHP {
    pub version: String,
    pub source: String,
    pub path: String,
    pub extensions: Option<Vec<String>>,
}

/// Information about a PATH conflict with another PHP provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellConflict {
    pub overriding_source: String,
    pub overriding_path: String,
}

/// Shell integration status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellIntegrationStatus {
    pub configured: bool,
    pub profile_path: Option<String>,
    pub burd_in_path: bool,
    pub conflict: Option<ShellConflict>,
}

/// Remote version info from static-php.dev
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemotePHPVersion {
    pub version: String,
    pub download_url: String,
    pub size: Option<u64>,
}

// === Directory Functions ===

/// Get the PVM directory where PHP versions are stored
pub fn get_pvm_dir() -> Result<PathBuf, String> {
    get_app_dir().map(|p| p.join("bin").join("php"))
}

/// Get the directory for a specific PHP version
pub fn get_version_dir(version: &str) -> Result<PathBuf, String> {
    get_pvm_dir().map(|p| p.join(version))
}

/// Get the path to the default symlink
pub fn get_default_link() -> Result<PathBuf, String> {
    get_pvm_dir().map(|p| p.join("default"))
}

/// Ensure the PVM directory exists
fn ensure_pvm_dir() -> Result<PathBuf, String> {
    let dir = get_pvm_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create PVM directory: {}", e))?;
    Ok(dir)
}

// === Shell Helpers ===

/// Execute a command in the user's login shell with their profile sourced.
/// This ensures we see the same PATH as a new terminal session would.
fn shell_exec(cmd: &str) -> Option<std::process::Output> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    let full_cmd = if let Some(profile_path) = get_shell_profile() {
        let profile_str = profile_path.to_string_lossy().to_string();
        format!("source \"{}\" 2>/dev/null; {}", profile_str, cmd)
    } else {
        cmd.to_string()
    };

    Command::new(&shell)
        .args(["-l", "-c", &full_cmd])
        .output()
        .ok()
}

// === Current PHP Detection ===

/// Detect the PHP source from a path
fn detect_php_source(path: &str) -> String {
    let path_lower = path.to_lowercase();

    if path_lower.contains("burd") || path_lower.contains("application support/burd") {
        "Burd".to_string()
    } else if path_lower.contains("herd") || path_lower.contains(".config/herd") {
        "Herd Pro".to_string()
    } else if path_lower.contains("homebrew")
        || path_lower.contains("/opt/homebrew")
        || path_lower.contains("/usr/local/opt")
        || path_lower.contains("/usr/local/cellar")
    {
        "Homebrew".to_string()
    } else if path == "/usr/bin/php" {
        "System".to_string()
    } else if path_lower.contains("mamp") {
        "MAMP".to_string()
    } else if path_lower.contains("xampp") {
        "XAMPP".to_string()
    } else {
        "Unknown".to_string()
    }
}

/// Parse PHP version from `php -v` output
fn parse_php_version(output: &str) -> Option<String> {
    // PHP 8.3.15 (cli) (built: ...)
    let re = Regex::new(r"PHP (\d+\.\d+\.\d+)").ok()?;
    re.captures(output)
        .and_then(|cap| cap.get(1))
        .map(|m| m.as_str().to_string())
}

/// Get the current PHP in the user's terminal PATH.
///
/// Spawns a login shell with the user's profile sourced to accurately detect
/// which PHP binary resolves in a new terminal session. Falls back to
/// detecting from the Tauri app's own environment if shell detection fails.
pub fn get_current_php() -> Option<CurrentPHP> {
    // Try shell-aware detection first (reflects actual terminal behavior)
    if let Some(php) = detect_terminal_php() {
        return Some(php);
    }

    // Fallback: detect from the Tauri app's own PATH
    detect_app_php()
}

/// Detect PHP from the user's terminal by spawning a login shell.
fn detect_terminal_php() -> Option<CurrentPHP> {
    let output = shell_exec(
        "PHP_BIN=$(which php 2>/dev/null); [ -n \"$PHP_BIN\" ] && printf 'PATH:%s\\n' \"$PHP_BIN\" && \"$PHP_BIN\" -v 2>/dev/null"
    )?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();

    let first_line = lines.next()?.trim();
    let path = first_line.strip_prefix("PATH:")?.to_string();
    if path.is_empty() {
        return None;
    }

    let remaining: String = lines.collect::<Vec<_>>().join("\n");
    let version = parse_php_version(&remaining)?;
    let source = detect_php_source(&path);

    Some(CurrentPHP {
        version,
        source,
        path,
        extensions: None,
    })
}

/// Detect PHP from the Tauri app's own environment (fallback).
fn detect_app_php() -> Option<CurrentPHP> {
    let which_output = Command::new("which").arg("php").output().ok()?;

    if !which_output.status.success() {
        return None;
    }

    let path = String::from_utf8_lossy(&which_output.stdout)
        .trim()
        .to_string();

    if path.is_empty() {
        return None;
    }

    let version_output = Command::new(&path).arg("-v").output().ok()?;

    let version_str = String::from_utf8_lossy(&version_output.stdout);
    let version = parse_php_version(&version_str)?;
    let source = detect_php_source(&path);

    Some(CurrentPHP {
        version,
        source,
        path,
        extensions: None,
    })
}

/// Get Burd's internal PHP (from ~/Library/Application Support/Burd/bin/php/default/php)
pub fn get_burd_php() -> Option<CurrentPHP> {
    let default_link = get_default_link().ok()?;
    let php_path = default_link.join("php");

    if !php_path.exists() {
        return None;
    }

    // Run php -v to get the version
    let version_output = Command::new(&php_path).arg("-v").output().ok()?;

    let version_str = String::from_utf8_lossy(&version_output.stdout);
    let version = parse_php_version(&version_str)?;

    // Run php -m to get the list of extensions
    let extensions = Command::new(&php_path)
        .arg("-m")
        .output()
        .ok()
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| {
                    let line = line.trim();
                    !line.is_empty() && !line.starts_with('[')
                })
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        });

    Some(CurrentPHP {
        version,
        source: "Burd".to_string(),
        path: php_path.to_string_lossy().to_string(),
        extensions,
    })
}

// === Installed Versions ===

/// Get the default PHP version (if set)
pub fn get_default_version() -> Option<String> {
    let link = get_default_link().ok()?;

    if !link.exists() {
        return None;
    }

    // Read the symlink target
    let target = fs::read_link(&link).ok()?;

    // Extract version from path (the directory name)
    target
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// List installed PHP versions
pub fn list_installed_versions() -> Result<Vec<PHPVersion>, String> {
    let pvm_dir = get_pvm_dir()?;

    if !pvm_dir.exists() {
        return Ok(Vec::new());
    }

    let default_version = get_default_version();
    let mut versions = Vec::new();

    let entries =
        fs::read_dir(&pvm_dir).map_err(|e| format!("Failed to read PVM directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip the "default" symlink
        if path.file_name().map(|n| n == "default").unwrap_or(false) {
            continue;
        }

        // Check if it's a directory with a php binary
        if path.is_dir() {
            let php_binary = path.join("php");
            if php_binary.exists() {
                if let Some(version) = path.file_name().and_then(|n| n.to_str()) {
                    let is_default = default_version
                        .as_ref()
                        .map(|d| d == version)
                        .unwrap_or(false);
                    versions.push(PHPVersion {
                        version: version.to_string(),
                        is_default,
                    });
                }
            }
        }
    }

    // Sort by version (newest first)
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));

    Ok(versions)
}

/// Compare two version strings (e.g., "8.4.12" vs "8.3.15")
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> { s.split('.').filter_map(|p| p.parse().ok()).collect() };

    let a_parts = parse(a);
    let b_parts = parse(b);

    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
        match a_part.cmp(b_part) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }

    a_parts.len().cmp(&b_parts.len())
}

// === Remote Versions ===

/// Fetch available PHP versions from static-php.dev
pub async fn list_remote_versions() -> Result<Vec<RemotePHPVersion>, String> {
    let arch = get_arch_string();
    let url = format!("{}/?format=json", DOWNLOAD_BASE_URL);

    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to fetch version list: {}", e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to fetch version list: HTTP {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    // Parse the JSON response - it's an array of file entries
    let entries: Vec<serde_json::Value> =
        serde_json::from_str(&body).map_err(|e| format!("Failed to parse response: {}", e))?;

    // Pattern to match PHP CLI binaries for our architecture
    // e.g., php-8.4.12-cli-macos-aarch64.tar.gz
    let pattern = format!(r"^php-(\d+\.\d+\.\d+)-cli-macos-{}\.tar\.gz$", arch);
    let re = Regex::new(&pattern).map_err(|e| format!("Failed to compile regex: {}", e))?;

    let installed = list_installed_versions().unwrap_or_default();
    let installed_versions: Vec<&str> = installed.iter().map(|v| v.version.as_str()).collect();

    let mut versions = Vec::new();

    for entry in entries {
        if let Some(name) = entry.get("name").and_then(|n| n.as_str()) {
            if let Some(caps) = re.captures(name) {
                if let Some(version_match) = caps.get(1) {
                    let version = version_match.as_str().to_string();

                    // Skip already installed versions
                    if installed_versions.contains(&version.as_str()) {
                        continue;
                    }

                    let download_url = format!("{}/{}", DOWNLOAD_BASE_URL, name);
                    let size = entry.get("size").and_then(|s| s.as_u64());

                    versions.push(RemotePHPVersion {
                        version,
                        download_url,
                        size,
                    });
                }
            }
        }
    }

    // Sort by version (newest first)
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));

    // Keep only the latest patch version per minor version
    let mut seen_minor: Vec<String> = Vec::new();
    versions.retain(|v| {
        let minor = get_minor_version(&v.version);
        if seen_minor.contains(&minor) {
            false
        } else {
            seen_minor.push(minor);
            true
        }
    });

    Ok(versions)
}

/// Get the minor version (e.g., "8.4" from "8.4.12")
fn get_minor_version(version: &str) -> String {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() >= 2 {
        format!("{}.{}", parts[0], parts[1])
    } else {
        version.to_string()
    }
}

/// Get the architecture string for downloads
fn get_arch_string() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "aarch64",
        "x86_64" => "x86_64",
        _ => "x86_64", // Fallback
    }
}

// === Download & Install ===

/// Download and install a PHP version
pub async fn download_version(version: &str, app_handle: &AppHandle) -> Result<(), String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let arch = get_arch_string();
    let filename = format!("php-{}-cli-macos-{}.tar.gz", version, arch);
    let url = format!("{}/{}", DOWNLOAD_BASE_URL, filename);

    // Ensure PVM directory exists
    ensure_pvm_dir()?;

    let version_dir = get_version_dir(version)?;

    // Check if already installed
    if version_dir.exists() {
        return Err(format!("PHP {} is already installed", version));
    }

    // Download the file
    let response = reqwest::get(&url)
        .await
        .map_err(|e| format!("Failed to download PHP {}: {}", version, e))?;

    if !response.status().is_success() {
        return Err(format!(
            "Failed to download PHP {}: HTTP {}",
            version,
            response.status()
        ));
    }

    let total_size = response.content_length().unwrap_or(0);

    // Create temp file
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(&filename);
    let mut file =
        fs::File::create(&temp_file).map_err(|e| format!("Failed to create temp file: {}", e))?;

    // Download with progress
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write to temp file: {}", e))?;

        downloaded += chunk.len() as u64;

        // Emit progress event
        let percentage = if total_size > 0 {
            (downloaded as f64 / total_size as f64 * 100.0) as u32
        } else {
            0
        };

        let _ = app_handle.emit(
            "php-download-progress",
            serde_json::json!({
                "version": version,
                "downloaded": downloaded,
                "total": total_size,
                "percentage": percentage,
            }),
        );
    }

    // Create version directory
    fs::create_dir_all(&version_dir)
        .map_err(|e| format!("Failed to create version directory: {}", e))?;

    // Extract the tarball
    let tar_gz =
        fs::File::open(&temp_file).map_err(|e| format!("Failed to open temp file: {}", e))?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);

    archive
        .unpack(&version_dir)
        .map_err(|e| format!("Failed to extract archive: {}", e))?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_file);

    // The archive might have the binary in root or in a subdirectory
    // Try to find the php binary
    let php_binary = find_php_binary(&version_dir)?;

    // Move the binary to the version directory root if needed
    let target_binary = version_dir.join("php");
    if php_binary != target_binary {
        fs::rename(&php_binary, &target_binary)
            .map_err(|e| format!("Failed to move PHP binary: {}", e))?;
    }

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target_binary)
            .map_err(|e| format!("Failed to get permissions: {}", e))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_binary, perms)
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Remove quarantine attribute on macOS
    let _ = Command::new("xattr")
        .args(["-d", "com.apple.quarantine"])
        .arg(&target_binary)
        .output();

    // Clean up any leftover directories from extraction
    cleanup_extraction(&version_dir)?;

    Ok(())
}

/// Find the php binary in the extracted directory
fn find_php_binary(dir: &PathBuf) -> Result<PathBuf, String> {
    // Check direct binary
    let direct = dir.join("php");
    if direct.exists() && direct.is_file() {
        return Ok(direct);
    }

    // Check for binary in subdirectories
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let nested = path.join("php");
                if nested.exists() && nested.is_file() {
                    return Ok(nested);
                }
                // Also check bin subdirectory
                let bin_nested = path.join("bin").join("php");
                if bin_nested.exists() && bin_nested.is_file() {
                    return Ok(bin_nested);
                }
            }
        }
    }

    Err("PHP binary not found in archive".to_string())
}

/// Clean up extraction artifacts (empty directories, etc.)
fn cleanup_extraction(version_dir: &PathBuf) -> Result<(), String> {
    if let Ok(entries) = fs::read_dir(version_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            // Remove any directories that aren't the php binary
            if path.is_dir() {
                let _ = fs::remove_dir_all(&path);
            }
        }
    }
    Ok(())
}

// === Version Management ===

/// Set the default PHP version
pub fn set_default_version(version: &str) -> Result<(), String> {
    let version_dir = get_version_dir(version)?;

    if !version_dir.exists() {
        return Err(format!("PHP {} is not installed", version));
    }

    // Verify the binary exists before switching
    let php_binary = version_dir.join("php");
    if !php_binary.exists() {
        return Err(format!("PHP binary not found for version {}", version));
    }

    let link = get_default_link()?;

    // Remove existing link
    let _ = fs::remove_file(&link);

    // Create new symlink
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&version_dir, &link)
            .map_err(|e| format!("Failed to create symlink: {}", e))?;
    }

    // Verify the symlink resolves correctly
    let resolved = link.join("php");
    if !resolved.exists() {
        return Err("Symlink created but PHP binary not accessible through it".to_string());
    }

    // Quick sanity check: run php -v to verify the binary works
    match Command::new(&resolved).arg("-v").output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "PHP {} binary exists but failed to run: {}",
                version,
                stderr.trim()
            ))
        }
        Err(e) => Err(format!("Failed to verify PHP {} binary: {}", version, e)),
    }
}

/// Delete an installed PHP version
pub fn delete_version(version: &str) -> Result<(), String> {
    let version_dir = get_version_dir(version)?;

    if !version_dir.exists() {
        return Err(format!("PHP {} is not installed", version));
    }

    // Check if this is the default version
    if let Some(default) = get_default_version() {
        if default == version {
            // Remove the default symlink first
            let _ = fs::remove_file(get_default_link()?);
        }
    }

    // Remove the version directory
    fs::remove_dir_all(&version_dir)
        .map_err(|e| format!("Failed to delete PHP {}: {}", version, e))?;

    Ok(())
}

// === Shell Integration ===

/// Get the path to the user's shell profile
fn get_shell_profile() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Check which shell is being used
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());

    if shell.contains("zsh") {
        // Check for .zshrc first, then .zprofile
        let zshrc = home.join(".zshrc");
        if zshrc.exists() {
            return Some(zshrc);
        }
        let zprofile = home.join(".zprofile");
        return Some(zprofile);
    } else if shell.contains("bash") {
        // Check for .bash_profile first, then .bashrc
        let bash_profile = home.join(".bash_profile");
        if bash_profile.exists() {
            return Some(bash_profile);
        }
        let bashrc = home.join(".bashrc");
        return Some(bashrc);
    }

    // Default to .zshrc on macOS
    Some(home.join(".zshrc"))
}

/// Get the PATH export line for Burd PHP
fn get_path_export_line() -> Result<String, String> {
    let pvm_dir = get_pvm_dir()?;
    let default_path = pvm_dir.join("default");
    Ok(format!("export PATH=\"{}:$PATH\"", default_path.display()))
}

/// Detect if another tool has overridden Burd PHP in the shell PATH.
///
/// Uses shell_exec() to spawn the user's actual login shell (respecting $SHELL)
/// with their profile sourced, accurately simulating what a new terminal sees.
fn detect_shell_conflict() -> Option<ShellConflict> {
    let output = shell_exec("which php")?;

    if !output.status.success() {
        return None;
    }

    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if resolved.is_empty() {
        return None;
    }

    let source = detect_php_source(&resolved);
    if source == "Burd" {
        return None;
    }

    Some(ShellConflict {
        overriding_source: source,
        overriding_path: resolved,
    })
}

/// Get shell integration status
pub fn get_shell_integration_status() -> ShellIntegrationStatus {
    let profile_path = get_shell_profile();
    let burd_in_path = is_burd_in_path();

    let configured = if let Some(ref path) = profile_path {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                content.contains(SHELL_MARKER)
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    let conflict = if configured {
        detect_shell_conflict()
    } else {
        None
    };

    ShellIntegrationStatus {
        configured,
        profile_path: profile_path.map(|p| p.to_string_lossy().to_string()),
        burd_in_path,
        conflict,
    }
}

/// Check if Burd PHP is currently in PATH
fn is_burd_in_path() -> bool {
    if let Ok(path) = std::env::var("PATH") {
        if let Ok(pvm_dir) = get_pvm_dir() {
            let default_path = pvm_dir.join("default").to_string_lossy().to_string();
            return path.contains(&default_path);
        }
    }
    false
}

/// Configure shell integration (add to profile)
pub fn configure_shell_integration() -> Result<(), String> {
    let profile =
        get_shell_profile().ok_or_else(|| "Could not determine shell profile path".to_string())?;

    let export_line = get_path_export_line()?;

    // Check if already configured
    let existing_content = if profile.exists() {
        fs::read_to_string(&profile).unwrap_or_default()
    } else {
        String::new()
    };

    if existing_content.contains(SHELL_MARKER) {
        return Ok(()); // Already configured
    }

    // Append the configuration
    let addition = format!("\n{}\n{}\n", SHELL_MARKER, export_line);

    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&profile)
        .map_err(|e| format!("Failed to open shell profile: {}", e))?;

    file.write_all(addition.as_bytes())
        .map_err(|e| format!("Failed to write to shell profile: {}", e))?;

    Ok(())
}

/// Remove shell integration
pub fn remove_shell_integration() -> Result<(), String> {
    let profile =
        get_shell_profile().ok_or_else(|| "Could not determine shell profile path".to_string())?;

    if !profile.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&profile).map_err(|e| format!("Failed to read shell profile: {}", e))?;

    // Remove our lines
    let new_content: Vec<&str> = content
        .lines()
        .filter(|line| !line.contains(SHELL_MARKER) && !line.contains("Burd/bin/php/default"))
        .collect();

    // Remove consecutive blank lines that might be left
    let cleaned = new_content.join("\n");

    fs::write(&profile, cleaned).map_err(|e| format!("Failed to write shell profile: {}", e))?;

    Ok(())
}

/// Reassert shell integration by moving Burd lines to the end of the profile.
///
/// This fixes PATH conflicts where another tool (e.g. Herd Pro) has added its
/// own PATH export after Burd's, causing it to take priority. The fix removes
/// existing Burd lines and re-appends them at the very end.
pub fn reassert_shell_integration() -> Result<(), String> {
    let profile =
        get_shell_profile().ok_or_else(|| "Could not determine shell profile path".to_string())?;

    if !profile.exists() {
        return Err("Shell profile does not exist".to_string());
    }

    let content =
        fs::read_to_string(&profile).map_err(|e| format!("Failed to read shell profile: {}", e))?;

    // Filter out existing Burd lines
    let filtered: Vec<&str> = content
        .lines()
        .filter(|line| !line.contains(SHELL_MARKER) && !line.contains("Burd/bin/php/default"))
        .collect();

    let mut new_content = filtered.join("\n");

    // Re-append Burd lines at the very end
    let export_line = get_path_export_line()?;
    new_content.push_str(&format!("\n{}\n{}\n", SHELL_MARKER, export_line));

    fs::write(&profile, new_content)
        .map_err(|e| format!("Failed to write shell profile: {}", e))?;

    Ok(())
}

// === Status ===

/// Get overall PVM status
pub fn get_pvm_status() -> PvmStatus {
    let pvm_dir = get_pvm_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let installed = list_installed_versions().unwrap_or_default();
    let default_version = get_default_version();
    let current_php = get_current_php();
    let burd_php = get_burd_php();

    // Check shell configuration by reading profile directly (avoids
    // spawning another shell — get_current_php already spawned one)
    let shell_configured = get_shell_profile()
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|content| content.contains(SHELL_MARKER))
        .unwrap_or(false);

    PvmStatus {
        pvm_dir,
        default_version,
        installed_count: installed.len(),
        current_php,
        burd_php,
        shell_configured,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert_eq!(
            compare_versions("8.4.12", "8.3.15"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("8.3.15", "8.3.14"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("8.3.15", "8.3.15"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_detect_php_source() {
        assert_eq!(detect_php_source("/opt/homebrew/bin/php"), "Homebrew");
        assert_eq!(
            detect_php_source("/Applications/Herd.app/Contents/Resources/php/8.3/bin/php"),
            "Herd Pro"
        );
        assert_eq!(detect_php_source("/usr/bin/php"), "System");
        assert_eq!(
            detect_php_source("/Users/test/Library/Application Support/Burd/bin/php/8.4.12/php"),
            "Burd"
        );
    }

    #[test]
    fn test_get_minor_version() {
        assert_eq!(get_minor_version("8.4.12"), "8.4");
        assert_eq!(get_minor_version("8.3.15"), "8.3");
    }
}
