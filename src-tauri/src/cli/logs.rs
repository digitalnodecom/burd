//! `burd logs [NAME] [--lines N] [--follow]` — recent logs for an instance.
//!
//! The daemon's `/instances/:id/logs` endpoint returns the full log buffer as
//! a string. `--lines` is applied client-side; `--follow` polls the endpoint
//! and prints new content as it arrives (no server-side streaming yet).

use crate::api_client::BurdApiClient;
use crate::cli::lifecycle::resolve_instance;
use crate::config::ConfigStore;
use std::thread;
use std::time::Duration;

pub struct LogsOptions {
    pub lines: usize,
    pub follow: bool,
}

impl Default for LogsOptions {
    fn default() -> Self {
        Self {
            lines: 100,
            follow: false,
        }
    }
}

pub fn run_logs(name: Option<String>, opts: LogsOptions) -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    let instance = resolve_instance(&config, name.as_deref())?;

    let client = BurdApiClient::new();
    if !client.is_available() {
        return Err(
            "Burd app isn't running. Open Burd or run `burd setup`, then try again.".to_string(),
        );
    }

    let path = format!("/instances/{}/logs", instance.id);
    let initial = client.get(&path)?;
    let logs = unwrap_log_body(&initial);
    let tail: Vec<&str> = logs.lines().collect();
    let start = tail.len().saturating_sub(opts.lines);
    let mut last_len = 0usize;
    for line in &tail[start..] {
        println!("{}", line);
        last_len = logs.len();
    }

    if !opts.follow {
        return Ok(());
    }

    // The API buffers logs in a single string; poll and print the suffix we
    // haven't seen. Server-side SSE/streaming would be better but isn't wired
    // up yet — 1s cadence is fine for dev tailing.
    loop {
        thread::sleep(Duration::from_secs(1));
        match client.get(&path) {
            Ok(body) => {
                let logs = unwrap_log_body(&body);
                if logs.len() > last_len {
                    print!("{}", &logs[last_len..]);
                    last_len = logs.len();
                } else if logs.len() < last_len {
                    // Log was truncated/rotated — reset and reprint everything.
                    println!("{}", logs);
                    last_len = logs.len();
                }
            }
            Err(e) => {
                eprintln!("Lost connection to Burd: {}", e);
                return Ok(());
            }
        }
    }
}

/// The API returns a JSON envelope `{ "data": "..." }`; `BurdApiClient` strips
/// `data` and pretty-prints the inner value. For a string, that produces
/// `"..."` with quotes/escapes. Unwrap that one level for readable output.
fn unwrap_log_body(body: &str) -> String {
    serde_json::from_str::<String>(body).unwrap_or_else(|_| body.to_string())
}
