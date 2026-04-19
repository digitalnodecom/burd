//! System tray / menu-bar integration.
//!
//! Builds a macOS-style tray icon with:
//! - Aggregate Burd status header
//! - Infrastructure services (DNS, Caddy proxy) with health dots
//! - Dynamic per-instance submenus (start/stop/restart/open/copy/logs/configure)
//! - Open Burd / Settings / Quit
//!
//! The menu is rebuilt (debounced) when `proxy-health-changed` or
//! `instances-changed` events fire. Click handlers invoke existing commands
//! via `AppHandle::state::<AppState>()` so state/event flow stays consistent.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Emitter, Listener, Manager};

use crate::commands::AppState;
use crate::error::LockExt;
use crate::lock;

const TRAY_ID: &str = "burd-tray";

#[derive(Copy, Clone, PartialEq, Eq)]
enum Health {
    Green,
    Amber,
    Red,
}

impl Health {
    fn dot(self) -> &'static str {
        match self {
            Health::Green => "🟢",
            Health::Amber => "🟡",
            Health::Red => "🔴",
        }
    }
}

/// Initialize the tray icon. Called from `setup()`.
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    let _tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(app.default_window_icon().unwrap().clone())
        .icon_as_template(true)
        .tooltip("Burd")
        .menu(&menu)
        .on_menu_event(|app, event| handle_menu_event(app, event))
        .build(app)?;

    spawn_refresh_listeners(app.clone());
    Ok(())
}

/// Listen to events that should trigger a menu rebuild. Debounced to coalesce
/// bursts (e.g. health poller + simultaneous instance start).
fn spawn_refresh_listeners(app: AppHandle) {
    let pending = Arc::new(AtomicBool::new(false));

    let trigger = {
        let app = app.clone();
        let pending = pending.clone();
        move || {
            if pending.swap(true, Ordering::SeqCst) {
                return;
            }
            let app = app.clone();
            let pending = pending.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(Duration::from_millis(150)).await;
                pending.store(false, Ordering::SeqCst);
                if let Err(e) = rebuild(&app) {
                    eprintln!("tray: rebuild failed: {e}");
                }
            });
        }
    };

    let t1 = trigger.clone();
    app.listen_any("proxy-health-changed", move |_| t1());
    let t2 = trigger.clone();
    app.listen_any("instances-changed", move |_| t2());
}

fn rebuild(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        tray.set_menu(Some(menu))?;
        update_tooltip(&tray, app);
    }
    Ok(())
}

fn update_tooltip(tray: &TrayIcon<tauri::Wry>, app: &AppHandle) {
    let health = aggregate_health(app);
    let label = match health {
        Health::Green => "Burd — all systems go",
        Health::Amber => "Burd — degraded",
        Health::Red => "Burd — issue",
    };
    let _ = tray.set_tooltip(Some(label));
}

/// Aggregate health: red if proxy poller returned false, amber if unknown,
/// green if healthy. Instance failures roll up to amber (non-fatal).
fn aggregate_health(app: &AppHandle) -> Health {
    let state = app.state::<AppState>();
    let proxy = state.proxy_healthy.load(Ordering::Relaxed);
    match proxy {
        1 => Health::Green,
        2 => Health::Red,
        _ => Health::Amber,
    }
}

// ---------------------------------------------------------------------------
// Menu construction
// ---------------------------------------------------------------------------

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    let health = aggregate_health(app);

    let header = MenuItem::with_id(
        app,
        "header",
        format!("Burd  {}", health.dot()),
        false,
        None::<&str>,
    )?;
    menu.append(&header)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    append_infrastructure(app, &menu)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    append_sites(app, &menu)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let open_app = MenuItem::with_id(app, "open-app", "Open Burd", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "open-settings", "Settings…", true, None::<&str>)?;
    menu.append(&open_app)?;
    menu.append(&settings)?;
    menu.append(&PredefinedMenuItem::separator(app)?)?;

    let quit = MenuItem::with_id(app, "quit", "Quit Burd", true, Some("Cmd+Q"))?;
    menu.append(&quit)?;

    Ok(menu)
}

fn append_infrastructure(app: &AppHandle, menu: &Menu<tauri::Wry>) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let proxy_code = state.proxy_healthy.load(Ordering::Relaxed);
    let proxy_dot = match proxy_code {
        1 => Health::Green.dot(),
        2 => Health::Red.dot(),
        _ => Health::Amber.dot(),
    };
    let proxy = MenuItem::with_id(
        app,
        "svc:proxy",
        format!("{} Caddy proxy", proxy_dot),
        false,
        None::<&str>,
    )?;
    menu.append(&proxy)?;

    let dns_running = state
        .dns_server
        .lock()
        .map(|s| s.is_running())
        .unwrap_or(false);
    let dns_dot = if dns_running {
        Health::Green.dot()
    } else {
        Health::Red.dot()
    };
    let dns = MenuItem::with_id(
        app,
        "svc:dns",
        format!("{} DNS server", dns_dot),
        false,
        None::<&str>,
    )?;
    menu.append(&dns)?;
    Ok(())
}

fn append_sites(app: &AppHandle, menu: &Menu<tauri::Wry>) -> tauri::Result<()> {
    let state = app.state::<AppState>();
    let snapshot = instance_snapshot(&state);

    let label = if snapshot.is_empty() {
        MenuItem::with_id(app, "sites-empty", "No instances yet", false, None::<&str>)?
    } else {
        MenuItem::with_id(app, "sites-header", "Sites", false, None::<&str>)?
    };
    menu.append(&label)?;

    for site in snapshot {
        let dot = if site.running {
            Health::Green.dot()
        } else {
            "⚫"
        };
        let title = if site.domain.is_empty() {
            format!("{} {}", dot, site.name)
        } else {
            format!("{} {}  ({})", dot, site.name, site.domain)
        };
        let sub = Submenu::with_id(app, format!("site:{}", site.id), title, true)?;

        if site.running {
            sub.append(&MenuItem::with_id(
                app,
                format!("site:{}:open", site.id),
                "Open in browser",
                !site.url().is_empty(),
                None::<&str>,
            )?)?;
            sub.append(&MenuItem::with_id(
                app,
                format!("site:{}:copy-url", site.id),
                "Copy URL",
                !site.url().is_empty(),
                None::<&str>,
            )?)?;
            sub.append(&PredefinedMenuItem::separator(app)?)?;
            sub.append(&MenuItem::with_id(
                app,
                format!("site:{}:restart", site.id),
                "Restart",
                true,
                None::<&str>,
            )?)?;
            sub.append(&MenuItem::with_id(
                app,
                format!("site:{}:stop", site.id),
                "Stop",
                true,
                None::<&str>,
            )?)?;
        } else {
            sub.append(&MenuItem::with_id(
                app,
                format!("site:{}:start", site.id),
                "Start",
                true,
                None::<&str>,
            )?)?;
        }

        sub.append(&PredefinedMenuItem::separator(app)?)?;
        sub.append(&MenuItem::with_id(
            app,
            format!("site:{}:logs", site.id),
            "View logs",
            true,
            None::<&str>,
        )?)?;
        sub.append(&MenuItem::with_id(
            app,
            format!("site:{}:configure", site.id),
            "Configure…",
            true,
            None::<&str>,
        )?)?;

        menu.append(&sub)?;
    }
    Ok(())
}

struct SiteRow {
    id: String,
    name: String,
    domain: String,
    port: u16,
    running: bool,
    ssl: bool,
}

impl SiteRow {
    fn url(&self) -> String {
        if self.domain.is_empty() {
            return String::new();
        }
        let scheme = if self.ssl { "https" } else { "http" };
        format!("{}://{}", scheme, self.domain)
    }
}

fn instance_snapshot(state: &AppState) -> Vec<SiteRow> {
    let (config, running_map) = match (lock!(state.config_store), lock!(state.process_manager)) {
        (Ok(cs), Ok(pm)) => {
            let cfg = match cs.load() {
                Ok(c) => c,
                Err(_) => return Vec::new(),
            };
            let running: std::collections::HashMap<uuid::Uuid, bool> = cfg
                .instances
                .iter()
                .map(|i| (i.id, pm.is_running(&i.id)))
                .collect();
            (cfg, running)
        }
        _ => return Vec::new(),
    };

    let tld = config.tld.clone();
    config
        .instances
        .iter()
        .map(|inst| {
            let mapped = config
                .domains
                .iter()
                .find(|d| d.routes_to_instance(&inst.id));
            let (domain, ssl) = match mapped {
                Some(d) => (d.full_domain(&tld), d.ssl_enabled),
                None => (String::new(), false),
            };
            SiteRow {
                id: inst.id.to_string(),
                name: inst.name.clone(),
                domain,
                port: inst.port,
                running: running_map.get(&inst.id).copied().unwrap_or(false),
                ssl,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Event handling
// ---------------------------------------------------------------------------

fn handle_menu_event(app: &AppHandle, event: MenuEvent) {
    let id = event.id.as_ref().to_string();

    match id.as_str() {
        "quit" => {
            app.exit(0);
            return;
        }
        "open-app" => {
            focus_main(app);
            return;
        }
        "open-settings" => {
            focus_main(app);
            let _ = app.emit("tray-navigate", serde_json::json!({ "section": "general" }));
            return;
        }
        _ => {}
    }

    // Site actions: "site:<uuid>:<action>"
    if let Some(rest) = id.strip_prefix("site:") {
        let mut parts = rest.splitn(2, ':');
        let uuid = parts.next().unwrap_or("").to_string();
        let action = parts.next().unwrap_or("");
        if uuid.is_empty() || action.is_empty() {
            return;
        }
        dispatch_site_action(app, uuid, action.to_string());
    }
}

fn focus_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

fn dispatch_site_action(app: &AppHandle, uuid: String, action: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let result: Result<(), String> = match action.as_str() {
            "start" => {
                crate::commands::start_instance(uuid.clone(), state.clone(), app.clone())
                    .await
                    .map(|_| ())
            }
            "stop" => {
                crate::commands::stop_instance(uuid.clone(), state.clone(), app.clone()).await
            }
            "restart" => {
                crate::commands::restart_instance(uuid.clone(), state.clone(), app.clone()).await
            }
            "open" => open_site_url(&app, &uuid).await,
            "copy-url" => copy_site_url(&app, &uuid).await,
            "logs" => {
                focus_main(&app);
                app.emit(
                    "tray-navigate",
                    serde_json::json!({ "section": "logs", "instanceId": uuid }),
                )
                .map_err(|e| e.to_string())
            }
            "configure" => {
                focus_main(&app);
                app.emit(
                    "tray-navigate",
                    serde_json::json!({ "section": "instances", "instanceId": uuid }),
                )
                .map_err(|e| e.to_string())
            }
            _ => Ok(()),
        };

        if let Err(e) = result {
            eprintln!("tray: action {} on {} failed: {}", action, uuid, e);
        }

        let _ = app.emit("instances-changed", serde_json::json!({}));
    });
}

async fn open_site_url(app: &AppHandle, uuid: &str) -> Result<(), String> {
    let url = site_url(app, uuid).ok_or_else(|| "no URL".to_string())?;
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| e.to_string())
}

async fn copy_site_url(app: &AppHandle, uuid: &str) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let url = site_url(app, uuid).ok_or_else(|| "no URL".to_string())?;
    app.clipboard().write_text(url).map_err(|e| e.to_string())
}

fn site_url(app: &AppHandle, uuid: &str) -> Option<String> {
    let state = app.state::<AppState>();
    let snapshot = instance_snapshot(&state);
    let parsed = uuid::Uuid::parse_str(uuid).ok()?;
    snapshot
        .into_iter()
        .find(|s| s.id == parsed.to_string())
        .map(|s| {
            if !s.domain.is_empty() {
                let scheme = if s.ssl { "https" } else { "http" };
                format!("{}://{}", scheme, s.domain)
            } else {
                format!("http://127.0.0.1:{}", s.port)
            }
        })
}
