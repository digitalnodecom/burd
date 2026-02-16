//! PM2 Process Manager Integration
//!
//! Provides integration with PM2 for managing Node.js applications.
//! Used primarily for Node-RED and other Node.js-based services.

use std::env;
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pm2Process {
    pub pm_id: u32,
    pub name: String,
    pub status: String,  // "online", "stopped", "errored"
    pub memory: u64,     // bytes
    pub cpu: f32,        // percentage
    pub uptime: Option<u64>,  // milliseconds
    pub restart_count: u32,
    pub script: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pm2Status {
    pub installed: bool,
    pub version: Option<String>,
    pub processes: Vec<Pm2Process>,
}

/// Get the user's default shell
fn get_user_shell() -> String {
    if let Ok(shell) = env::var("SHELL") {
        return shell;
    }
    if cfg!(target_os = "macos") {
        "/bin/zsh".to_string()
    } else {
        "/bin/bash".to_string()
    }
}

/// Get NVM initialization script if available
pub fn get_nvm_init_script() -> String {
    let home = env::var("HOME").unwrap_or_else(|_| "/Users/tj".to_string());

    // Check for Homebrew NVM first
    let homebrew_paths = [
        "/opt/homebrew/opt/nvm/nvm.sh",
        "/usr/local/opt/nvm/nvm.sh",
    ];

    for path in &homebrew_paths {
        if std::path::Path::new(path).exists() {
            return format!(
                r#"export NVM_DIR="$HOME/.nvm" && source "{}" && "#,
                path
            );
        }
    }

    // Check standard NVM location
    let nvm_sh = format!("{}/.nvm/nvm.sh", home);
    if std::path::Path::new(&nvm_sh).exists() {
        return format!(
            r#"export NVM_DIR="$HOME/.nvm" && source "{}" && "#,
            nvm_sh
        );
    }

    // No NVM found, just use PATH as-is
    String::new()
}

/// Run a PM2 command
fn run_pm2_command(args: &str) -> Result<String, String> {
    let home = env::var("HOME").unwrap_or_else(|_| "/Users/tj".to_string());
    let nvm_init = get_nvm_init_script();

    // Build command that initializes NVM (if available) then runs pm2
    let script = format!(
        r#"export HOME="{}" && {}pm2 {}"#,
        home, nvm_init, args
    );

    let shell = get_user_shell();
    // Use -i for interactive mode so NVM/shell profiles load properly
    // This is needed because GUI apps don't inherit the user's shell environment
    let output = Command::new(&shell)
        .args(["-i", "-c", &script])
        .env("HOME", &home)
        .output()
        .map_err(|e| format!("Failed to run {}: {}", shell, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stdout)
    } else {
        // Check if PM2 is not found
        if stderr.contains("command not found") || stderr.contains("not found") {
            Err("PM2 not installed".to_string())
        } else {
            Err(format!("{}{}", stderr, stdout))
        }
    }
}

/// Check if PM2 is installed
pub fn is_pm2_installed() -> bool {
    run_pm2_command("--version").is_ok()
}

/// Get PM2 version
pub fn get_pm2_version() -> Option<String> {
    run_pm2_command("--version")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Get PM2 status including version and running processes
pub fn get_pm2_status() -> Pm2Status {
    let version = get_pm2_version();
    let installed = version.is_some();

    let processes = if installed {
        list_processes().unwrap_or_default()
    } else {
        Vec::new()
    };

    Pm2Status {
        installed,
        version,
        processes,
    }
}

/// List all PM2 processes
pub fn list_processes() -> Result<Vec<Pm2Process>, String> {
    let output = run_pm2_command("jlist")?;

    // Parse JSON output
    let json: Vec<serde_json::Value> = serde_json::from_str(&output)
        .map_err(|e| format!("Failed to parse PM2 output: {}", e))?;

    let processes = json.iter().filter_map(|proc| {
        let pm_id = proc.get("pm_id")?.as_u64()? as u32;
        let name = proc.get("name")?.as_str()?.to_string();

        // Get status from pm2_env
        let pm2_env = proc.get("pm2_env")?;
        let status = pm2_env.get("status")?.as_str()?.to_string();
        let restart_count = pm2_env.get("restart_time").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        let pm_uptime = pm2_env.get("pm_uptime").and_then(|v| v.as_u64());
        let script = pm2_env.get("pm_exec_path").and_then(|v| v.as_str()).map(|s| s.to_string());

        // Get memory and CPU from monit
        let monit = proc.get("monit");
        let memory = monit.and_then(|m| m.get("memory")).and_then(|v| v.as_u64()).unwrap_or(0);
        let cpu = monit.and_then(|m| m.get("cpu")).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

        // Calculate uptime in milliseconds
        let uptime = pm_uptime.map(|start| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            now.saturating_sub(start)
        });

        Some(Pm2Process {
            pm_id,
            name,
            status,
            memory,
            cpu,
            uptime,
            restart_count,
            script,
        })
    }).collect();

    Ok(processes)
}

/// Install PM2 globally via npm
pub fn install_pm2() -> Result<String, String> {
    let home = env::var("HOME").unwrap_or_else(|_| "/Users/tj".to_string());
    let nvm_init = get_nvm_init_script();

    if nvm_init.is_empty() {
        return Err("Node.js is required to install PM2. Please install Node.js via NVM first.".to_string());
    }

    let script = format!(
        r#"export HOME="{}" && {}npm install -g pm2"#,
        home, nvm_init
    );

    let shell = get_user_shell();
    // Use -i for interactive mode so NVM loads properly
    let output = Command::new(&shell)
        .args(["-i", "-c", &script])
        .env("HOME", &home)
        .output()
        .map_err(|e| format!("Failed to run npm: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(format!("{}{}", stdout, stderr))
    } else {
        Err(format!("{}{}", stderr, stdout))
    }
}

/// Start an application with PM2
pub fn start_app(name: &str, script: &str, args: Option<&str>, cwd: Option<&str>) -> Result<String, String> {
    let mut cmd = format!("start {} --name \"{}\"", script, name);

    if let Some(working_dir) = cwd {
        cmd.push_str(&format!(" --cwd \"{}\"", working_dir));
    }

    if let Some(app_args) = args {
        cmd.push_str(&format!(" -- {}", app_args));
    }

    run_pm2_command(&cmd)
}

/// Stop a PM2 process by name or ID
pub fn stop_app(name_or_id: &str) -> Result<String, String> {
    run_pm2_command(&format!("stop \"{}\"", name_or_id))
}

/// Restart a PM2 process by name or ID
pub fn restart_app(name_or_id: &str) -> Result<String, String> {
    run_pm2_command(&format!("restart \"{}\"", name_or_id))
}

/// Delete a PM2 process by name or ID
pub fn delete_app(name_or_id: &str) -> Result<String, String> {
    run_pm2_command(&format!("delete \"{}\"", name_or_id))
}

/// Get logs for a PM2 process
pub fn get_logs(name_or_id: &str, lines: u32) -> Result<String, String> {
    run_pm2_command(&format!("logs \"{}\" --lines {} --nostream", name_or_id, lines))
}

/// Save PM2 process list (for resurrection on reboot)
pub fn save() -> Result<String, String> {
    run_pm2_command("save")
}

/// Stop all PM2 processes
pub fn stop_all() -> Result<String, String> {
    run_pm2_command("stop all")
}

/// Delete all PM2 processes
pub fn delete_all() -> Result<String, String> {
    run_pm2_command("delete all")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_pm2_installed() {
        // Just test that the function runs without panicking
        let _ = is_pm2_installed();
    }
}
