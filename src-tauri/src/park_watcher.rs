//! Park Directory Watcher
//!
//! Watches parked directories for file system changes and triggers sync.

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEventKind};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

/// State for managing file system watchers
pub struct ParkWatcherState {
    /// Map of parked directory ID to its watcher
    watchers: Arc<Mutex<HashMap<Uuid, WatcherHandle>>>,
}

struct WatcherHandle {
    #[allow(dead_code)]
    debouncer: notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>,
    #[allow(dead_code)]
    path: PathBuf,
}

impl Default for ParkWatcherState {
    fn default() -> Self {
        Self::new()
    }
}

impl ParkWatcherState {
    pub fn new() -> Self {
        Self {
            watchers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Start watching a parked directory
    pub fn start_watching(
        &self,
        parked_dir_id: Uuid,
        path: PathBuf,
        app_handle: AppHandle,
    ) -> Result<(), String> {
        let mut watchers = self
            .watchers
            .lock()
            .map_err(|e| format!("Failed to lock watchers: {}", e))?;

        // Stop existing watcher for this directory if any
        watchers.remove(&parked_dir_id);

        // Create a debounced watcher (300ms debounce)
        let id = parked_dir_id;
        let app = app_handle.clone();
        let mut debouncer = new_debouncer(
            Duration::from_millis(300),
            move |res: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
                if let Ok(events) = res {
                    // Filter for directory creation/deletion events
                    let has_dir_change = events
                        .iter()
                        .any(|e| matches!(e.kind, DebouncedEventKind::Any) && e.path.is_dir());

                    if has_dir_change {
                        // Emit event to frontend
                        let _ = app.emit(
                            "park:directory-changed",
                            serde_json::json!({
                                "parked_dir_id": id.to_string(),
                            }),
                        );
                    }
                }
            },
        )
        .map_err(|e| format!("Failed to create watcher: {}", e))?;

        // Start watching the directory (non-recursive - only watch immediate children)
        debouncer
            .watcher()
            .watch(&path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;

        watchers.insert(parked_dir_id, WatcherHandle { debouncer, path });

        Ok(())
    }

    /// Stop watching a parked directory
    pub fn stop_watching(&self, parked_dir_id: Uuid) -> Result<(), String> {
        let mut watchers = self
            .watchers
            .lock()
            .map_err(|e| format!("Failed to lock watchers: {}", e))?;

        watchers.remove(&parked_dir_id);

        Ok(())
    }

    /// Stop all watchers
    #[allow(dead_code)]
    pub fn stop_all(&self) -> Result<(), String> {
        let mut watchers = self
            .watchers
            .lock()
            .map_err(|e| format!("Failed to lock watchers: {}", e))?;

        watchers.clear();
        Ok(())
    }

    /// Get the number of active watchers
    #[allow(dead_code)]
    pub fn watcher_count(&self) -> usize {
        self.watchers.lock().map(|w| w.len()).unwrap_or(0)
    }
}

/// Initialize watchers for all existing parked directories on startup
pub fn init_watchers(
    watcher_state: &ParkWatcherState,
    parked_directories: Vec<(Uuid, PathBuf)>,
    app_handle: AppHandle,
) {
    for (id, path) in parked_directories {
        let _ = watcher_state.start_watching(id, path, app_handle.clone());
    }
}
