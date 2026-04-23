//! `burd start|stop|restart [NAME]` — instance lifecycle from the CLI.
//!
//! When NAME is provided, looks up an instance by:
//!   1. exact instance name
//!   2. domain subdomain (e.g. `myapp` → the instance backing `myapp.<tld>`)
//!
//! When NAME is omitted, resolves the instance tied to the current directory
//! (document_root match — same strategy used by `burd secure`, `burd open`, etc.).

use crate::api_client::BurdApiClient;
use crate::config::{ConfigStore, DomainTarget, Instance};
use std::env;
use std::path::Path;
use uuid::Uuid;

pub fn run_start(name: Option<String>) -> Result<(), String> {
    dispatch(name, Action::Start)
}

pub fn run_stop(name: Option<String>) -> Result<(), String> {
    dispatch(name, Action::Stop)
}

pub fn run_restart(name: Option<String>) -> Result<(), String> {
    dispatch(name, Action::Restart)
}

#[derive(Copy, Clone)]
enum Action {
    Start,
    Stop,
    Restart,
}

impl Action {
    fn verb(self) -> &'static str {
        match self {
            Action::Start => "start",
            Action::Stop => "stop",
            Action::Restart => "restart",
        }
    }
    fn past(self) -> &'static str {
        match self {
            Action::Start => "started",
            Action::Stop => "stopped",
            Action::Restart => "restarted",
        }
    }
}

fn dispatch(name: Option<String>, action: Action) -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let instance = resolve_instance(&config, name.as_deref())?;

    let client = BurdApiClient::new();
    if !client.is_available() {
        return Err(
            "Burd app isn't running. Open Burd or run `burd setup`, then try again."
                .to_string(),
        );
    }

    let path = format!("/instances/{}/{}", instance.id, action.verb());
    match client.post(&path, &serde_json::json!({})) {
        Ok(_) => {
            println!("✓ {} '{}'", capitalize(action.past()), instance.name);
            Ok(())
        }
        // Repeat start/stop against an already-{running,stopped} instance is
        // a no-op success for CLI ergonomics — otherwise scripts that call
        // `burd start` unconditionally would crash on the second run.
        Err(msg) if is_idempotent_noop(action, &msg) => {
            println!(
                "• '{}' is already {}. Nothing to do.",
                instance.name,
                match action {
                    Action::Start | Action::Restart => "running",
                    Action::Stop => "stopped",
                }
            );
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn is_idempotent_noop(action: Action, msg: &str) -> bool {
    let lower = msg.to_lowercase();
    match action {
        Action::Start => lower.contains("already running"),
        Action::Stop => lower.contains("already stopped") || lower.contains("not running"),
        Action::Restart => false,
    }
}

pub(crate) fn resolve_instance(
    config: &crate::config::Config,
    name: Option<&str>,
) -> Result<Instance, String> {
    if let Some(raw) = name {
        let tld_suffix = format!(".{}", config.tld);
        let stripped = raw.strip_suffix(&tld_suffix).unwrap_or(raw);

        if let Some(inst) = config.instances.iter().find(|i| i.name == stripped) {
            return Ok(inst.clone());
        }
        if let Ok(uuid) = Uuid::parse_str(stripped) {
            if let Some(inst) = config.instances.iter().find(|i| i.id == uuid) {
                return Ok(inst.clone());
            }
        }
        let subdomain = slug::slugify(stripped);
        if let Some(domain) = config.domains.iter().find(|d| d.subdomain == subdomain) {
            if let DomainTarget::Instance(instance_id) = &domain.target {
                if let Some(inst) = config.instances.iter().find(|i| &i.id == instance_id) {
                    return Ok(inst.clone());
                }
            }
        }
        return Err(format!("No instance matches '{}'.", raw));
    }

    // No name — resolve from the current directory.
    let cwd = env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let cwd_str = cwd.to_string_lossy().to_string();

    let inst = config.instances.iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == cwd_str || Path::new(dr).starts_with(&cwd))
            .unwrap_or(false)
    });

    inst.cloned().ok_or_else(|| {
        format!(
            "No Burd instance for this directory ({}).\n\
             Run `burd init` here to create one, or pass a name: `burd <command> <name>`.",
            cwd.display()
        )
    })
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}
