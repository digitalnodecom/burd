//! Configuration module
//!
//! Handles application configuration, data models, and path utilities.

mod models;
mod paths;
mod store;

// Re-export models
pub use models::{
    BinaryInfo, Config, Domain, DomainSource, DomainTarget, Instance, ParkedDirectory, ServiceType,
    // Tunnel types (re-exported from tunnel module)
    FrpServer, Tunnel, TunnelTarget, SubdomainConfig, TunnelState, TunnelWithState,
    // Stack types
    Stack, StackExport, StackService, StackDomain, StackRequirements,
    StackImportPreview, MissingVersion, ImportConflict, ConflictResolution, ImportResult,
};

// Re-export store
pub use store::ConfigStore;

// Re-export path utilities
pub use paths::{
    get_app_dir, get_bin_dir, get_pids_dir, get_instances_dir, get_instance_dir,
    get_binary_name, get_binary_path, get_versioned_binary_path,
    get_versioned_binary_dir, get_service_bin_dir,
};
