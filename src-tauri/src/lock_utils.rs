//! Lock acquisition utilities
//!
//! Provides helper functions for acquiring locks on AppState components with
//! consistent error handling. This reduces boilerplate and improves code readability.

use std::sync::{MutexGuard, PoisonError};
use crate::commands::AppState;
use crate::config::ConfigStore;
use crate::process::ProcessManager;
use crate::binary::BinaryManager;
use crate::dns::DnsServer;

/// Helper function to acquire config_store lock with consistent error handling
///
/// # Example
/// ```no_run
/// let config = with_config_lock(&state, |store| {
///     store.load()
/// })?;
/// ```
pub fn with_config_lock<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&ConfigStore) -> Result<R, String>,
{
    let guard = state.config_store
        .lock()
        .map_err(|e| format!("Failed to acquire config store lock: {}", e))?;
    f(&guard)
}

/// Helper function to acquire a mutable config_store lock
///
/// # Example
/// ```no_run
/// with_config_lock_mut(&state, |store| {
///     store.create_instance(name, port, service_type, version, config)
/// })?;
/// ```
pub fn with_config_lock_mut<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&mut ConfigStore) -> Result<R, String>,
{
    let mut guard = state.config_store
        .lock()
        .map_err(|e| format!("Failed to acquire config store lock: {}", e))?;
    f(&mut guard)
}

/// Helper function to acquire process_manager lock with consistent error handling
///
/// # Example
/// ```no_run
/// let is_running = with_process_lock(&state, |pm| {
///     Ok(pm.is_running(&instance_id))
/// })?;
/// ```
pub fn with_process_lock<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&ProcessManager) -> Result<R, String>,
{
    let guard = state.process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;
    f(&guard)
}

/// Helper function to acquire a mutable process_manager lock
///
/// # Example
/// ```no_run
/// with_process_lock_mut(&state, |pm| {
///     pm.start(&instance_id, binary_path, args, env_vars, data_dir)
/// })?;
/// ```
pub fn with_process_lock_mut<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&mut ProcessManager) -> Result<R, String>,
{
    let mut guard = state.process_manager
        .lock()
        .map_err(|e| format!("Failed to acquire process manager lock: {}", e))?;
    f(&mut guard)
}

/// Helper function to acquire binary_manager lock with consistent error handling
///
/// # Example
/// ```no_run
/// let versions = with_binary_lock(&state, |bm| {
///     bm.get_installed_versions_sync(service_type)
/// })?;
/// ```
pub fn with_binary_lock<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&BinaryManager) -> Result<R, String>,
{
    let guard = state.binary_manager
        .lock()
        .map_err(|e| format!("Failed to acquire binary manager lock: {}", e))?;
    f(&guard)
}

/// Helper function to acquire a mutable binary_manager lock
///
/// # Example
/// ```no_run
/// with_binary_lock_mut(&state, |bm| {
///     bm.install(service_type, version)
/// })?;
/// ```
pub fn with_binary_lock_mut<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&mut BinaryManager) -> Result<R, String>,
{
    let mut guard = state.binary_manager
        .lock()
        .map_err(|e| format!("Failed to acquire binary manager lock: {}", e))?;
    f(&mut guard)
}

/// Helper function to acquire dns_server lock with consistent error handling
///
/// # Example
/// ```no_run
/// let port = with_dns_lock(&state, |dns| {
///     Ok(dns.port())
/// })?;
/// ```
pub fn with_dns_lock<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&DnsServer) -> Result<R, String>,
{
    let guard = state.dns_server
        .lock()
        .map_err(|e| format!("Failed to acquire DNS server lock: {}", e))?;
    f(&guard)
}

/// Helper function to acquire a mutable dns_server lock
///
/// # Example
/// ```no_run
/// with_dns_lock_mut(&state, |dns| {
///     dns.start()
/// })?;
/// ```
pub fn with_dns_lock_mut<F, R>(state: &AppState, f: F) -> Result<R, String>
where
    F: FnOnce(&mut DnsServer) -> Result<R, String>,
{
    let mut guard = state.dns_server
        .lock()
        .map_err(|e| format!("Failed to acquire DNS server lock: {}", e))?;
    f(&mut guard)
}

/// Helper to get a direct lock guard (for cases where the closure pattern doesn't work)
///
/// This is useful when you need to hold the lock across multiple operations or
/// when the lock needs to be dropped explicitly.
pub fn lock_config_store(state: &AppState) -> Result<MutexGuard<'_, ConfigStore>, String> {
    state.config_store
        .lock()
        .map_err(|e: PoisonError<MutexGuard<ConfigStore>>| {
            format!("Failed to acquire config store lock: {}", e)
        })
}

/// Helper to get a direct process manager lock guard
pub fn lock_process_manager(state: &AppState) -> Result<MutexGuard<'_, ProcessManager>, String> {
    state.process_manager
        .lock()
        .map_err(|e: PoisonError<MutexGuard<ProcessManager>>| {
            format!("Failed to acquire process manager lock: {}", e)
        })
}

/// Helper to get a direct binary manager lock guard
pub fn lock_binary_manager(state: &AppState) -> Result<MutexGuard<'_, BinaryManager>, String> {
    state.binary_manager
        .lock()
        .map_err(|e: PoisonError<MutexGuard<BinaryManager>>| {
            format!("Failed to acquire binary manager lock: {}", e)
        })
}

/// Helper to get a direct DNS server lock guard
pub fn lock_dns_server(state: &AppState) -> Result<MutexGuard<'_, DnsServer>, String> {
    state.dns_server
        .lock()
        .map_err(|e: PoisonError<MutexGuard<DnsServer>>| {
            format!("Failed to acquire DNS server lock: {}", e)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigStore;
    use crate::process::ProcessManager;
    use crate::binary::BinaryManager;
    use crate::dns::DnsServer;
    use crate::proxy::ProxyServer;
    use std::sync::{Arc, Mutex};
    use tokio::sync::Mutex as AsyncMutex;

    fn create_test_state() -> AppState {
        let config_store = ConfigStore::new().unwrap();
        let process_manager = ProcessManager::new();
        let binary_manager = BinaryManager::new();
        let dns_server = DnsServer::new(5300, "test".to_string());
        let proxy_server = ProxyServer::new(8080, "test".to_string());

        AppState {
            config_store: Arc::new(std::sync::Mutex::new(config_store)),
            process_manager: Arc::new(std::sync::Mutex::new(process_manager)),
            binary_manager: Arc::new(std::sync::Mutex::new(binary_manager)),
            dns_server: Arc::new(std::sync::Mutex::new(dns_server)),
            proxy_server: Arc::new(AsyncMutex::new(proxy_server)),
        }
    }

    #[test]
    fn test_with_config_lock() {
        let state = create_test_state();
        let result = with_config_lock(&state, |store| {
            let config = store.load()?;
            Ok(config.tld.clone())
        });
        assert!(result.is_ok());
    }

    #[test]
    fn test_with_process_lock() {
        let state = create_test_state();
        let result = with_process_lock(&state, |pm| {
            let instance_id = uuid::Uuid::new_v4();
            Ok(pm.is_running(&instance_id))
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), false);
    }

    #[test]
    fn test_with_dns_lock() {
        let state = create_test_state();
        let result = with_dns_lock(&state, |dns| {
            Ok(dns.port())
        });
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 5300);
    }

    #[test]
    fn test_lock_config_store_direct() {
        let state = create_test_state();
        let result = lock_config_store(&state);
        assert!(result.is_ok());
        let guard = result.unwrap();
        let config = guard.load().unwrap();
        // TLD defaults to "burd" from domain module DEFAULT_TLD
        assert!(!config.tld.is_empty());
    }

    #[test]
    fn test_lock_process_manager_direct() {
        let state = create_test_state();
        let result = lock_process_manager(&state);
        assert!(result.is_ok());
    }
}
