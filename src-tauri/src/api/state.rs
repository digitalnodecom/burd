//! API state management

use crate::commands::AppState;
use std::sync::Arc;

/// Wrapper around AppState for API handlers.
///
/// This provides a clonable state type that can be used with Axum's State extractor.
#[derive(Clone)]
pub struct ApiState {
    pub inner: Arc<AppState>,
}

impl ApiState {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self { inner: app_state }
    }
}
