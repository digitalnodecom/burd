//! One-click MCP registration into AI client configs.
//!
//! Burd already exposes an MCP server via `burd mcp`; historically users had to
//! copy-paste config into each AI client by hand. These commands detect the
//! supported clients on the machine and write (or remove) a `burd` MCP server
//! entry in their config files — idempotently, with a one-time backup, and
//! without disturbing any other servers the user has configured.
//!
//! macOS paths only for now; the client table is structured so other platforms
//! can be added later.

use crate::constants::CLI_INSTALL_PATH;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

/// A supported AI client and how to reach its MCP config.
struct ClientSpec {
    /// Stable identifier used by the frontend.
    id: &'static str,
    /// Human-readable name.
    name: &'static str,
    /// Config file, relative to the user's home directory.
    config_rel: &'static str,
    /// Top-level JSON key that holds the server map. Claude clients and Cursor
    /// use `mcpServers`; VS Code uses `servers`.
    servers_key: &'static str,
    /// Whether this client wants a `"type": "stdio"` marker on each entry.
    needs_type: bool,
    /// Extra home-relative paths that, if present, indicate the client is
    /// installed even when its config file doesn't exist yet.
    detect_rel: &'static [&'static str],
}

const CLIENTS: &[ClientSpec] = &[
    ClientSpec {
        id: "claude-code",
        name: "Claude Code",
        config_rel: ".claude.json",
        servers_key: "mcpServers",
        needs_type: false,
        detect_rel: &[".claude", ".claude.json"],
    },
    ClientSpec {
        id: "claude-desktop",
        name: "Claude Desktop",
        config_rel: "Library/Application Support/Claude/claude_desktop_config.json",
        servers_key: "mcpServers",
        needs_type: false,
        detect_rel: &["Library/Application Support/Claude"],
    },
    ClientSpec {
        id: "cursor",
        name: "Cursor",
        config_rel: ".cursor/mcp.json",
        servers_key: "mcpServers",
        needs_type: false,
        detect_rel: &[".cursor"],
    },
    ClientSpec {
        id: "vscode",
        name: "VS Code",
        config_rel: "Library/Application Support/Code/User/mcp.json",
        servers_key: "servers",
        needs_type: true,
        detect_rel: &["Library/Application Support/Code"],
    },
];

/// Status of one client, returned to the frontend.
#[derive(Debug, Serialize)]
pub struct McpClientState {
    pub id: String,
    pub name: String,
    /// The client appears to be installed on this machine.
    pub installed: bool,
    /// The client's config already contains a `burd` MCP server entry.
    pub connected: bool,
    /// Absolute path to the config file Burd would write.
    pub config_path: String,
}

fn home() -> Result<PathBuf, String> {
    dirs::home_dir().ok_or_else(|| "Could not resolve the home directory".to_string())
}

fn spec(id: &str) -> Result<&'static ClientSpec, String> {
    CLIENTS
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("Unknown MCP client '{}'", id))
}

/// The command Burd registers: the installed CLI if present, else the plain
/// name (which must be on PATH). Callers should ensure the CLI is installed.
fn burd_command() -> String {
    if std::path::Path::new(CLI_INSTALL_PATH).exists() {
        CLI_INSTALL_PATH.to_string()
    } else {
        "burd".to_string()
    }
}

/// Build the server entry value for a given client shape.
fn burd_entry(spec: &ClientSpec) -> Value {
    let mut entry = Map::new();
    if spec.needs_type {
        entry.insert("type".to_string(), json!("stdio"));
    }
    entry.insert("command".to_string(), json!(burd_command()));
    entry.insert("args".to_string(), json!(["mcp"]));
    Value::Object(entry)
}

/// Read a JSON config file, returning an empty object when it doesn't exist.
fn read_config(path: &PathBuf) -> Result<Value, String> {
    match std::fs::read_to_string(path) {
        Ok(s) if s.trim().is_empty() => Ok(json!({})),
        Ok(s) => serde_json::from_str(&s)
            .map_err(|e| format!("{} is not valid JSON: {}", path.display(), e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(e) => Err(format!("Couldn't read {}: {}", path.display(), e)),
    }
}

/// Atomically write JSON: temp file in the same dir, then rename.
fn write_config(path: &PathBuf, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Couldn't create {}: {}", parent.display(), e))?;
    }
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|e| format!("Couldn't serialize config: {}", e))?;
    text.push('\n');

    let tmp = path.with_extension("burd.tmp");
    std::fs::write(&tmp, text.as_bytes())
        .map_err(|e| format!("Couldn't write {}: {}", tmp.display(), e))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("Couldn't finalize {}: {}", path.display(), e))?;
    Ok(())
}

/// True when `config[servers_key]["burd"]` exists.
fn has_burd(config: &Value, servers_key: &str) -> bool {
    config
        .get(servers_key)
        .and_then(|s| s.get("burd"))
        .is_some()
}

/// List every supported client with install / connection status.
#[tauri::command]
pub fn detect_mcp_clients() -> Result<Vec<McpClientState>, String> {
    let home = home()?;
    let mut out = Vec::with_capacity(CLIENTS.len());

    for c in CLIENTS {
        let config_path = home.join(c.config_rel);
        let config_exists = config_path.exists();
        let installed = config_exists || c.detect_rel.iter().any(|p| home.join(p).exists());

        let connected = if config_exists {
            read_config(&config_path)
                .map(|cfg| has_burd(&cfg, c.servers_key))
                .unwrap_or(false)
        } else {
            false
        };

        out.push(McpClientState {
            id: c.id.to_string(),
            name: c.name.to_string(),
            installed,
            connected,
            config_path: config_path.to_string_lossy().to_string(),
        });
    }

    Ok(out)
}

/// Register the `burd` MCP server in one client's config. Idempotent; leaves
/// other servers untouched; backs the original file up once to `<file>.burd.bak`.
#[tauri::command]
pub fn install_mcp_client(client: String) -> Result<String, String> {
    let spec = spec(&client)?;
    let path = home()?.join(spec.config_rel);

    // One-time backup of a pre-existing, user-authored file.
    if path.exists() {
        let backup = path.with_extension("burd.bak");
        if !backup.exists() {
            std::fs::copy(&path, &backup)
                .map_err(|e| format!("Couldn't back up {}: {}", path.display(), e))?;
        }
    }

    let mut config = read_config(&path)?;
    if !config.is_object() {
        return Err(format!(
            "{} has an unexpected shape (expected a JSON object)",
            path.display()
        ));
    }

    let obj = config.as_object_mut().unwrap();
    let servers = obj
        .entry(spec.servers_key.to_string())
        .or_insert_with(|| json!({}));
    let servers_obj = servers.as_object_mut().ok_or_else(|| {
        format!(
            "\"{}\" in {} is not an object",
            spec.servers_key,
            path.display()
        )
    })?;
    servers_obj.insert("burd".to_string(), burd_entry(spec));

    write_config(&path, &config)?;
    Ok(format!(
        "Connected Burd to {}. Restart {} to load it.",
        spec.name, spec.name
    ))
}

/// Remove the `burd` MCP server entry from one client's config. No-op when the
/// file or entry is absent.
#[tauri::command]
pub fn uninstall_mcp_client(client: String) -> Result<String, String> {
    let spec = spec(&client)?;
    let path = home()?.join(spec.config_rel);
    if !path.exists() {
        return Ok(format!("{} has no Burd entry.", spec.name));
    }

    let mut config = read_config(&path)?;
    let mut removed = false;
    if let Some(servers) = config
        .get_mut(spec.servers_key)
        .and_then(|s| s.as_object_mut())
    {
        removed = servers.remove("burd").is_some();
    }

    if removed {
        write_config(&path, &config)?;
        Ok(format!("Disconnected Burd from {}.", spec.name))
    } else {
        Ok(format!("{} had no Burd entry.", spec.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_shapes_match_client() {
        let cc = spec("claude-code").unwrap();
        let e = burd_entry(cc);
        assert!(e.get("type").is_none());
        assert_eq!(e["args"], json!(["mcp"]));

        let vs = spec("vscode").unwrap();
        assert_eq!(burd_entry(vs)["type"], json!("stdio"));
    }

    #[test]
    fn has_burd_detects_entry() {
        let cfg = json!({ "mcpServers": { "burd": { "command": "burd" } } });
        assert!(has_burd(&cfg, "mcpServers"));
        assert!(!has_burd(&cfg, "servers"));
        assert!(!has_burd(&json!({}), "mcpServers"));
    }
}
