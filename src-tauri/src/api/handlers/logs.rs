//! Logs API handlers (system-wide log access)

use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;

use crate::api::{state::ApiState, types::ApiResponse};
use crate::logs::{
    get_caddy_log_path, get_instance_log_path, get_last_lines, get_log_sources_with_instances,
    parse_caddy_json, parse_plain_text, LogEntry, LogSourceInfo,
};

pub async fn sources(State(state): State<ApiState>) -> Json<ApiResponse<Vec<LogSourceInfo>>> {
    let cs = match state.inner.config_store.lock() {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
    };
    let cfg = match cs.load() {
        Ok(c) => c,
        Err(e) => return Json(ApiResponse::err(e.to_string())),
    };
    Json(ApiResponse::ok(get_log_sources_with_instances(&cfg.instances)))
}

#[derive(Deserialize)]
pub struct RecentQuery {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn recent(
    State(state): State<ApiState>,
    Query(q): Query<RecentQuery>,
) -> Json<ApiResponse<Vec<LogEntry>>> {
    let limit = q.limit.unwrap_or(200);
    let instances = {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        match cs.load() {
            Ok(c) => c.instances.clone(),
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        }
    };

    let mut all = Vec::new();
    let want_source = q.source.as_deref();

    let want_caddy = want_source.is_none() || want_source == Some("caddy");
    if want_caddy {
        let path = get_caddy_log_path();
        if path.exists() {
            if let Ok(lines) = get_last_lines(path.to_str().unwrap_or(""), limit) {
                for line in lines {
                    if let Some(entry) = parse_caddy_json(&line) {
                        all.push(entry);
                    }
                }
            }
        }
    }

    for instance in &instances {
        let svc = instance.service_type.as_str();
        if svc == "caddy" {
            continue;
        }
        if let Some(s) = want_source {
            if s != svc {
                continue;
            }
        }
        if let Ok(p) = get_instance_log_path(&instance.id.to_string()) {
            if p.exists() {
                if let Ok(lines) = get_last_lines(p.to_str().unwrap_or(""), limit) {
                    for line in lines {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let mut entry = parse_plain_text(
                                trimmed,
                                svc,
                                Some(&instance.id.to_string()),
                            );
                            entry.domain = Some(instance.name.clone());
                            all.push(entry);
                        }
                    }
                }
            }
        }
    }

    all.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    all.truncate(limit);
    Json(ApiResponse::ok(all))
}
