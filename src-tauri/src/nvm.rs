//! NVM (Node Version Manager) Integration
//!
//! Provides integration with NVM for managing Node.js versions.
//! Supports version installation, switching, and listing available versions.

use std::env;
use std::path::PathBuf;
use std::process::Command;
use serde::{Deserialize, Serialize};

/// Strip ANSI escape codes from a string
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we hit a letter (end of escape sequence)
            while let Some(&next) = chars.peek() {
                chars.next();
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeVersion {
    pub version: String,
    pub is_lts: bool,
    pub lts_name: Option<String>,
    pub is_current: bool,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NvmStatus {
    pub installed: bool,
    pub nvm_dir: Option<String>,
    pub current_version: Option<String>,
    pub default_version: Option<String>,
}

/// Get the NVM directory path
fn get_nvm_dir() -> Option<PathBuf> {
    // Check NVM_DIR env var first
    if let Ok(nvm_dir) = env::var("NVM_DIR") {
        let path = PathBuf::from(&nvm_dir);
        if path.exists() {
            return Some(path);
        }
    }

    // Fall back to default location
    if let Ok(home) = env::var("HOME") {
        let default_path = PathBuf::from(&home).join(".nvm");
        if default_path.exists() {
            return Some(default_path);
        }
    }

    None
}

/// Get the path to nvm.sh - handles both standard and Homebrew installations
fn get_nvm_sh_path() -> Option<PathBuf> {
    // Check Homebrew location first (common on macOS)
    let homebrew_paths = [
        "/opt/homebrew/opt/nvm/nvm.sh",
        "/usr/local/opt/nvm/nvm.sh",
    ];

    for path in &homebrew_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }

    // Check standard NVM_DIR location
    if let Some(nvm_dir) = get_nvm_dir() {
        let nvm_sh = nvm_dir.join("nvm.sh");
        if nvm_sh.exists() {
            // Check if it's a symlink and resolve it
            if let Ok(resolved) = std::fs::read_link(&nvm_sh) {
                if resolved.exists() {
                    return Some(resolved);
                }
            }
            return Some(nvm_sh);
        }
    }

    None
}

/// Check if NVM is installed
pub fn is_nvm_installed() -> bool {
    get_nvm_sh_path().is_some()
}

/// Get the user's default shell
fn get_user_shell() -> String {
    // Try SHELL environment variable first
    if let Ok(shell) = env::var("SHELL") {
        return shell;
    }
    // Default to zsh on macOS, bash elsewhere
    if cfg!(target_os = "macos") {
        "/bin/zsh".to_string()
    } else {
        "/bin/bash".to_string()
    }
}

/// Run an NVM command by sourcing nvm.sh first
fn run_nvm_command(args: &str) -> Result<String, String> {
    let nvm_sh = get_nvm_sh_path().ok_or_else(|| "NVM not found".to_string())?;

    // Get HOME directory - critical for NVM to work correctly
    let home = env::var("HOME").unwrap_or_else(|_| {
        // Fallback: try to get from user info
        dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/Users/tj".to_string())
    });

    let nvm_dir = get_nvm_dir().unwrap_or_else(|| {
        PathBuf::from(&home).join(".nvm")
    });

    // Build the command that exports HOME, NVM_DIR, unsets conflicting vars, sources nvm.sh and runs the nvm command
    // npm_config_prefix conflicts with NVM - must be unset
    let script = format!(
        r#"export HOME="{}" && export NVM_DIR="{}" && unset npm_config_prefix && source "{}" && nvm {}"#,
        home,
        nvm_dir.display(),
        nvm_sh.display(),
        args
    );

    // Use the user's default shell (zsh on modern macOS, bash on Linux)
    let shell = get_user_shell();
    let output = Command::new(&shell)
        .args(["-c", &script])
        .env("HOME", &home)  // Also set in environment
        .output()
        .map_err(|e| format!("Failed to run {}: {}", shell, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        // Strip ANSI codes from output and combine stdout/stderr
        let combined = format!("{}{}", stdout, stderr);
        Ok(strip_ansi_codes(&combined))
    } else {
        Err(format!("NVM command failed: {}{}", stderr, stdout))
    }
}

/// Get NVM status including current and default versions
pub fn get_nvm_status() -> NvmStatus {
    let installed = is_nvm_installed();

    if !installed {
        return NvmStatus {
            installed: false,
            nvm_dir: None,
            current_version: None,
            default_version: None,
        };
    }

    let nvm_dir = get_nvm_dir().map(|p| p.to_string_lossy().to_string());

    // Get current version
    let current_version = run_nvm_command("current")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && v != "none" && v != "system");

    // Get default alias
    // Output format: "default -> 21 (-> v21.x.y)" or "default -> 21 (-> N/A)"
    // We want to extract the resolved version (vX.Y.Z) if available
    let default_version = run_nvm_command("alias default")
        .ok()
        .and_then(|output| {
            // First try to find the resolved version in parentheses: (-> vX.Y.Z)
            if let Some(start) = output.find("(->") {
                let after_arrow = &output[start + 3..];
                if let Some(version) = after_arrow
                    .split_whitespace()
                    .find(|s| s.starts_with('v') && s.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false))
                {
                    let clean = version.trim_end_matches([')', '*']);
                    if !clean.is_empty() && clean != "N/A" {
                        return Some(clean.to_string());
                    }
                }
            }
            // Fall back to the alias target (e.g., "21" or "node")
            output.split("->")
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });

    NvmStatus {
        installed,
        nvm_dir,
        current_version,
        default_version,
    }
}

/// List installed Node versions
pub fn list_installed_versions() -> Result<Vec<NodeVersion>, String> {
    let output = run_nvm_command("ls --no-colors")?;
    let current = run_nvm_command("current").ok().map(|v| v.trim().to_string());
    let default_alias = run_nvm_command("alias default").ok();

    // Parse the default version from alias output
    let default_version = default_alias.and_then(|output| {
        // Format: "default -> 21 (-> v21.x.y)" or "default -> v21.x.y"
        output.split("->")
            .last()
            .and_then(|s| {
                // Find any vX.Y.Z pattern in the string
                let trimmed = s.trim().trim_matches(|c| c == '(' || c == ')' || c == '*');
                trimmed.split_whitespace()
                    .find(|part| part.starts_with('v') && part.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false))
                    .map(|v| v.trim_end_matches('*').to_string())
            })
    });

    let mut versions = Vec::new();

    for line in output.lines() {
        let line = line.trim();

        // Skip empty lines
        if line.is_empty() {
            continue;
        }

        // Skip alias lines (format: "name -> target")
        // These include: default ->, latest ->, node ->, stable ->, lts/* ->, lts/name ->
        if line.contains(" -> ") && !line.starts_with("->") {
            continue;
        }

        // Skip system line
        if line.contains("system") {
            continue;
        }

        // Installed versions end with * and start with optional -> and version
        // Format: "v18.17.0 *" or "->     v20.18.0 *" or "       v22.16.0 *"
        if !line.ends_with('*') {
            continue;
        }

        // Extract version - find vX.Y.Z pattern
        let version_str = line
            .replace("->", "")
            .split_whitespace()
            .find(|s| s.starts_with('v') && s.chars().nth(1).map(|c| c.is_ascii_digit()).unwrap_or(false))
            .map(|s| s.trim_end_matches('*').to_string());

        if let Some(version) = version_str {
            // Determine if current version
            let is_current = line.starts_with("->") ||
                current.as_ref().map(|c| c == &version).unwrap_or(false);
            let is_default = default_version.as_ref().map(|d| d == &version).unwrap_or(false);

            // Avoid duplicates
            if !versions.iter().any(|v: &NodeVersion| v.version == version) {
                versions.push(NodeVersion {
                    version,
                    is_lts: false, // We'll enrich this from remote data if needed
                    lts_name: None,
                    is_current,
                    is_default,
                });
            }
        }
    }

    // Sort versions in descending order (newest first)
    versions.sort_by(|a, b| {
        compare_versions(&b.version, &a.version)
    });

    Ok(versions)
}

/// List available remote LTS versions
pub fn list_remote_lts_versions() -> Result<Vec<NodeVersion>, String> {
    let output = run_nvm_command("ls-remote --lts --no-colors")?;
    let installed = list_installed_versions().unwrap_or_default();
    let installed_versions: Vec<&str> = installed.iter().map(|v| v.version.as_str()).collect();

    let mut versions = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Parse lines like "       v18.20.5   (LTS: Hydrogen)"
        let version_str = line
            .split_whitespace()
            .find(|s| s.starts_with('v'))
            .map(|s| s.to_string());

        if let Some(version) = version_str {
            // Skip if already installed
            if installed_versions.contains(&version.as_str()) {
                continue;
            }

            // Extract LTS name
            let lts_name = line
                .split("LTS:")
                .nth(1)
                .and_then(|s| s.split(')').next())
                .map(|s| s.trim().to_string());

            // Only include LTS versions and avoid duplicates
            if lts_name.is_some() && !versions.iter().any(|v: &NodeVersion| v.version == version) {
                versions.push(NodeVersion {
                    version,
                    is_lts: true,
                    lts_name,
                    is_current: false,
                    is_default: false,
                });
            }
        }
    }

    // Sort versions in descending order (newest first) and take only latest per LTS codename
    versions.sort_by(|a, b| compare_versions(&b.version, &a.version));

    // Keep only the latest version per LTS codename
    let mut seen_lts: Vec<String> = Vec::new();
    versions.retain(|v| {
        if let Some(ref name) = v.lts_name {
            if seen_lts.contains(name) {
                false
            } else {
                seen_lts.push(name.clone());
                true
            }
        } else {
            true
        }
    });

    Ok(versions)
}

/// Install a Node version
pub fn install_version(version: &str) -> Result<String, String> {
    run_nvm_command(&format!("install {}", version))
}

/// Uninstall a Node version
pub fn uninstall_version(version: &str) -> Result<String, String> {
    let clean_version = version.trim();
    let result = run_nvm_command(&format!("uninstall {}", clean_version));

    // Check if the output indicates failure (NVM doesn't always exit with error code)
    if let Ok(ref output) = result {
        let lower = output.to_lowercase();
        if lower.contains("cannot uninstall") || lower.contains("is not installed") {
            return Err(output.clone());
        }
    }

    result
}

/// Set the default Node version
pub fn set_default_version(version: &str) -> Result<String, String> {
    run_nvm_command(&format!("alias default {}", version))
}

/// Compare two version strings (e.g., "v20.18.0" vs "v18.20.5")
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions() {
        assert_eq!(compare_versions("v20.18.0", "v18.20.5"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("v18.20.5", "v18.20.4"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("v18.20.5", "v18.20.5"), std::cmp::Ordering::Equal);
    }
}
