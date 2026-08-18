//! Native macOS application menu bar.
//!
//! Builds the standard app / Edit / Window menus (so the usual shortcuts —
//! copy/paste, hide, minimize, Cmd+Q — keep working) and adds a Mac-standard
//! "Check for Updates…" item under the app menu. Clicking it emits
//! `menu:check-for-updates`, which the frontend updater handles.

use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Wry};

/// Menu item id for "Check for Updates…".
pub const CHECK_FOR_UPDATES_ID: &str = "check-for-updates";

/// Build the application menu, set it, and wire up the check-for-updates event.
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let check_updates = MenuItem::with_id(
        app,
        CHECK_FOR_UPDATES_ID,
        "Check for Updates…",
        true,
        None::<&str>,
    )?;

    // First submenu becomes the app menu on macOS (labelled with the app name).
    let app_menu = Submenu::with_items(
        app,
        "Burd",
        true,
        &[
            &PredefinedMenuItem::about(app, Some("Burd"), Some(AboutMetadata::default()))?,
            &PredefinedMenuItem::separator(app)?,
            &check_updates,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::show_all(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, None)?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;

    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::close_window(app, None)?,
        ],
    )?;

    let menu: Menu<Wry> = Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])?;
    app.set_menu(menu)?;

    // Predefined items (about, quit, copy, …) are handled by the OS; only the
    // custom "Check for Updates…" needs routing to the frontend.
    app.on_menu_event(|app, event| {
        if event.id.as_ref() == CHECK_FOR_UPDATES_ID {
            let _ = app.emit("menu:check-for-updates", ());
        }
    });

    Ok(())
}
