//! API state management

use crate::commands::AppState;
use std::sync::Arc;
use tauri::AppHandle;

/// Wrapper around AppState for API handlers.
///
/// This provides a clonable state type that can be used with Axum's State extractor.
/// `app_handle` is `Some` when the API runs inside the Tauri process; absent in tests.
#[derive(Clone)]
pub struct ApiState {
    pub inner: Arc<AppState>,
    pub app_handle: Option<AppHandle>,
}

impl ApiState {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self { inner: app_state, app_handle: None }
    }

    pub fn with_app_handle(mut self, handle: AppHandle) -> Self {
        self.app_handle = Some(handle);
        self
    }
}
