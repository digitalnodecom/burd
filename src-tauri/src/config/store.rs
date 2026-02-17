//! Configuration store
//!
//! Handles loading, saving, and CRUD operations for the config file.

use chrono::Utc;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use super::{
    get_instance_dir, BinaryInfo, Config, Domain, DomainTarget, FrpServer, Instance,
    ParkedDirectory, ServiceType, Stack, SubdomainConfig, Tunnel, TunnelTarget,
};

pub struct ConfigStore {
    config_path: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Result<Self, String> {
        let app_dir = super::get_app_dir()?;
        fs::create_dir_all(&app_dir)
            .map_err(|e| format!("Failed to create app directory: {}", e))?;

        Ok(Self {
            config_path: app_dir.join("config.json"),
        })
    }

    pub fn load(&self) -> Result<Config, String> {
        if !self.config_path.exists() {
            return Ok(Config::default());
        }

        let content = fs::read_to_string(&self.config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        let mut config: Config =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

        // Migrate legacy instance fields
        let mut needs_save = false;
        for instance in &mut config.instances {
            if instance.master_key.is_some() {
                instance.migrate();
                needs_save = true;
            }
        }

        // Save if migration occurred (uses the atomic save method)
        if needs_save {
            self.save(&config)?;
        }

        Ok(config)
    }

    pub fn save(&self, config: &Config) -> Result<(), String> {
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        // Atomic write: write to temp file, then rename
        // This prevents data corruption if process crashes mid-write
        let temp_path = self.config_path.with_extension("json.tmp");

        fs::write(&temp_path, &content)
            .map_err(|e| format!("Failed to write temp config: {}", e))?;

        fs::rename(&temp_path, &self.config_path)
            .map_err(|e| format!("Failed to rename config: {}", e))
    }

    // ========================================================================
    // Instance Management
    // ========================================================================

    pub fn create_instance(
        &self,
        name: String,
        port: u16,
        service_type: ServiceType,
        version: String,
        service_config: serde_json::Value,
        custom_domain: Option<String>,
    ) -> Result<Instance, String> {
        let mut config = self.load()?;

        // Check if port is already in use by another instance
        if config.instances.iter().any(|i| i.port == port) {
            return Err(format!("Port {} is already used by another instance", port));
        }

        let instance = Instance {
            id: Uuid::new_v4(),
            name,
            port,
            service_type,
            version,
            config: service_config,
            master_key: None,
            auto_start: false,
            created_at: Utc::now(),
            domain: custom_domain,
            domain_enabled: true,
            stack_id: None,
        };

        // Create instance data directory
        let instance_dir = get_instance_dir(&instance.id)?;
        fs::create_dir_all(&instance_dir)
            .map_err(|e| format!("Failed to create instance directory: {}", e))?;

        // Create initial Caddyfile for FrankenPHP Park
        if service_type == ServiceType::FrankenPhpPark {
            let caddyfile_path = instance_dir.join("Caddyfile");
            let initial_caddyfile = format!(
                r#"{{
    frankenphp
    order php_server before file_server
}}

:{} {{
    handle {{
        respond "FrankenPHP Park is running. Add a parked directory to serve your projects." 200
    }}
}}
"#,
                port
            );
            fs::write(&caddyfile_path, initial_caddyfile)
                .map_err(|e| format!("Failed to create initial Caddyfile: {}", e))?;
        }

        config.instances.push(instance.clone());
        self.save(&config)?;

        Ok(instance)
    }

    pub fn delete_instance(&self, id: Uuid) -> Result<(), String> {
        let mut config = self.load()?;

        let idx = config
            .instances
            .iter()
            .position(|i| i.id == id)
            .ok_or_else(|| format!("Instance {} not found", id))?;

        config.instances.remove(idx);
        self.save(&config)?;

        // Optionally remove data directory (we keep it for safety)
        // let instance_dir = get_instance_dir(&id)?;
        // let _ = fs::remove_dir_all(instance_dir);

        Ok(())
    }

    pub fn get_instance(&self, id: Uuid) -> Result<Instance, String> {
        let config = self.load()?;
        config
            .instances
            .into_iter()
            .find(|i| i.id == id)
            .ok_or_else(|| format!("Instance {} not found", id))
    }

    /// Update instance domain settings
    pub fn update_instance_domain(
        &self,
        id: Uuid,
        domain: Option<String>,
        enabled: bool,
    ) -> Result<Instance, String> {
        let mut config = self.load()?;

        let instance = config
            .instances
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| format!("Instance {} not found", id))?;

        instance.domain = domain;
        instance.domain_enabled = enabled;

        let updated = instance.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Update instance configuration
    pub fn update_instance_config(
        &self,
        id: Uuid,
        new_config: serde_json::Value,
    ) -> Result<Instance, String> {
        let mut config = self.load()?;

        let instance = config
            .instances
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| format!("Instance {} not found", id))?;

        instance.config = new_config;

        let updated = instance.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Update instance version
    pub fn update_instance_version(
        &self,
        id: Uuid,
        new_version: String,
    ) -> Result<Instance, String> {
        let mut config = self.load()?;

        let instance = config
            .instances
            .iter_mut()
            .find(|i| i.id == id)
            .ok_or_else(|| format!("Instance {} not found", id))?;

        instance.version = new_version;

        let updated = instance.clone();
        self.save(&config)?;

        Ok(updated)
    }

    // ========================================================================
    // Binary Management
    // ========================================================================

    /// Update binary info for a specific service and version
    pub fn update_binary_info(
        &self,
        service_type: ServiceType,
        info: BinaryInfo,
    ) -> Result<(), String> {
        let mut config = self.load()?;
        let version = info.version.clone();
        config
            .binaries
            .entry(service_type)
            .or_insert_with(HashMap::new)
            .insert(version, info);
        self.save(&config)
    }

    /// Get binary info for a specific version (or latest if version not found)
    pub fn get_binary_info(
        &self,
        service_type: ServiceType,
        version: Option<&str>,
    ) -> Result<Option<BinaryInfo>, String> {
        let config = self.load()?;
        let versions = match config.binaries.get(&service_type) {
            Some(v) => v,
            None => return Ok(None),
        };

        if let Some(ver) = version {
            // Return specific version
            Ok(versions.get(ver).cloned())
        } else {
            // Return most recent (any version)
            Ok(versions.values().next().cloned())
        }
    }

    /// Get all installed versions for a service type
    pub fn get_installed_versions(&self, service_type: ServiceType) -> Result<Vec<String>, String> {
        let config = self.load()?;
        Ok(config
            .binaries
            .get(&service_type)
            .map(|v| v.keys().cloned().collect())
            .unwrap_or_default())
    }

    /// Get all binary info for a service type (all versions)
    pub fn get_all_binary_info(
        &self,
        service_type: ServiceType,
    ) -> Result<HashMap<String, BinaryInfo>, String> {
        let config = self.load()?;
        Ok(config
            .binaries
            .get(&service_type)
            .cloned()
            .unwrap_or_default())
    }

    /// Remove a specific version of a binary
    pub fn remove_binary_version(
        &self,
        service_type: ServiceType,
        version: &str,
    ) -> Result<(), String> {
        let mut config = self.load()?;
        if let Some(versions) = config.binaries.get_mut(&service_type) {
            versions.remove(version);
            // Remove the service entry if no versions remain
            if versions.is_empty() {
                config.binaries.remove(&service_type);
            }
        }
        self.save(&config)
    }

    // ========================================================================
    // Settings
    // ========================================================================

    /// Update the TLD setting
    pub fn update_tld(&self, tld: String) -> Result<(), String> {
        let mut config = self.load()?;
        config.tld = tld;
        self.save(&config)
    }

    /// Update the proxy_installed setting
    pub fn set_proxy_installed(&self, installed: bool) -> Result<(), String> {
        let mut config = self.load()?;
        config.proxy_installed = installed;
        self.save(&config)
    }

    // ========================================================================
    // Domain Management
    // ========================================================================

    /// Get all domains
    pub fn list_domains(&self) -> Result<Vec<Domain>, String> {
        let config = self.load()?;
        Ok(config.domains)
    }

    /// Get a specific domain by ID
    pub fn get_domain(&self, id: Uuid) -> Result<Domain, String> {
        let config = self.load()?;
        config
            .domains
            .into_iter()
            .find(|d| d.id == id)
            .ok_or_else(|| format!("Domain {} not found", id))
    }

    /// Create a new domain routing to an instance
    pub fn create_domain_for_instance(
        &self,
        subdomain: String,
        instance_id: Uuid,
        ssl_enabled: bool,
    ) -> Result<Domain, String> {
        let mut config = self.load()?;

        // Validate instance exists
        if !config.instances.iter().any(|i| i.id == instance_id) {
            return Err(format!("Instance {} not found", instance_id));
        }

        // Check for duplicate subdomain
        if config.domains.iter().any(|d| d.subdomain == subdomain) {
            return Err(format!("Domain '{}' already exists", subdomain));
        }

        let domain = Domain::for_instance(subdomain, instance_id, ssl_enabled);

        config.domains.push(domain.clone());
        self.save(&config)?;

        Ok(domain)
    }

    /// Create a new domain routing to a raw port
    pub fn create_domain_for_port(
        &self,
        subdomain: String,
        port: u16,
        ssl_enabled: bool,
    ) -> Result<Domain, String> {
        let mut config = self.load()?;

        // Check for duplicate subdomain
        if config.domains.iter().any(|d| d.subdomain == subdomain) {
            return Err(format!("Domain '{}' already exists", subdomain));
        }

        let domain = Domain::for_port(subdomain, port, ssl_enabled);

        config.domains.push(domain.clone());
        self.save(&config)?;

        Ok(domain)
    }

    /// Create a new domain serving static files from a directory
    pub fn create_domain_for_static_files(
        &self,
        subdomain: String,
        path: String,
        browse: bool,
        ssl_enabled: bool,
    ) -> Result<Domain, String> {
        let mut config = self.load()?;

        // Check for duplicate subdomain
        if config.domains.iter().any(|d| d.subdomain == subdomain) {
            return Err(format!("Domain '{}' already exists", subdomain));
        }

        // Validate that the path exists and is a directory
        let path_buf = std::path::PathBuf::from(&path);
        if !path_buf.exists() {
            return Err(format!("Path '{}' does not exist", path));
        }
        if !path_buf.is_dir() {
            return Err(format!("Path '{}' is not a directory", path));
        }

        let domain = Domain::for_static_files(subdomain, path, browse, ssl_enabled);

        config.domains.push(domain.clone());
        self.save(&config)?;

        Ok(domain)
    }

    /// Update an existing domain
    pub fn update_domain(
        &self,
        id: Uuid,
        subdomain: Option<String>,
        target: Option<DomainTarget>,
    ) -> Result<Domain, String> {
        let mut config = self.load()?;

        // First, find the domain index and current subdomain
        let domain_idx = config
            .domains
            .iter()
            .position(|d| d.id == id)
            .ok_or_else(|| format!("Domain {} not found", id))?;

        let current_subdomain = config.domains[domain_idx].subdomain.clone();

        // Check for duplicate subdomain if changing (before mutable borrow)
        if let Some(ref new_subdomain) = subdomain {
            if new_subdomain != &current_subdomain
                && config
                    .domains
                    .iter()
                    .any(|d| d.id != id && d.subdomain == *new_subdomain)
            {
                return Err(format!("Domain '{}' already exists", new_subdomain));
            }
        }

        // Validate instance target if changing
        if let Some(DomainTarget::Instance(instance_id)) = &target {
            if !config.instances.iter().any(|i| &i.id == instance_id) {
                return Err(format!("Instance {} not found", instance_id));
            }
        }

        // Now do the mutable updates
        let domain = &mut config.domains[domain_idx];

        if let Some(new_subdomain) = subdomain {
            domain.subdomain = new_subdomain;
        }
        if let Some(new_target) = target {
            domain.target = new_target;
        }

        let updated = domain.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Delete a domain by ID
    pub fn delete_domain(&self, id: Uuid) -> Result<(), String> {
        let mut config = self.load()?;

        let idx = config
            .domains
            .iter()
            .position(|d| d.id == id)
            .ok_or_else(|| format!("Domain {} not found", id))?;

        // Get the domain before removing it so we can clear instance.domain if needed
        let domain_to_delete = &config.domains[idx];
        let subdomain_to_clear = domain_to_delete.subdomain.clone();

        // If this domain routes to an instance, clear the instance's domain field if it matches
        if let DomainTarget::Instance(instance_id) = &domain_to_delete.target {
            if let Some(instance) = config.instances.iter_mut().find(|i| &i.id == instance_id) {
                // Clear instance.domain if it matches the deleted domain's subdomain
                if instance.domain.as_ref() == Some(&subdomain_to_clear) {
                    instance.domain = None;
                }
            }
        }

        config.domains.remove(idx);
        self.save(&config)?;

        Ok(())
    }

    /// Update SSL enabled status for a domain
    pub fn update_domain_ssl(&self, id: Uuid, ssl_enabled: bool) -> Result<Domain, String> {
        let mut config = self.load()?;

        let domain = config
            .domains
            .iter_mut()
            .find(|d| d.id == id)
            .ok_or_else(|| format!("Domain {} not found", id))?;

        domain.ssl_enabled = ssl_enabled;

        let updated = domain.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Delete all domains that route to a specific instance
    pub fn delete_domains_for_instance(&self, instance_id: Uuid) -> Result<Vec<Domain>, String> {
        let mut config = self.load()?;

        let (removed, remaining): (Vec<_>, Vec<_>) = config
            .domains
            .into_iter()
            .partition(|d| d.routes_to_instance(&instance_id));

        config.domains = remaining;
        self.save(&config)?;

        Ok(removed)
    }

    /// Find domain by subdomain
    pub fn find_domain_by_subdomain(&self, subdomain: &str) -> Result<Option<Domain>, String> {
        let config = self.load()?;
        Ok(config
            .domains
            .into_iter()
            .find(|d| d.subdomain == subdomain))
    }

    /// Migrate existing instance domain settings to Domain entities
    /// This is called once on startup to migrate from old format
    pub fn migrate_instance_domains(&self) -> Result<Vec<Domain>, String> {
        let mut config = self.load()?;
        let mut migrated = Vec::new();

        for instance in &config.instances {
            // Only migrate if domain_enabled and no domain already exists for this instance
            if instance.domain_enabled {
                // Check if a domain already exists for this specific instance (by instance ID)
                // This prevents recreating domains that were manually deleted
                let has_domain = config
                    .domains
                    .iter()
                    .any(|d| matches!(&d.target, DomainTarget::Instance(id) if id == &instance.id));

                if !has_domain {
                    let subdomain = instance.effective_domain_slug();
                    // Migrated domains default to SSL enabled
                    let domain = Domain::for_instance(subdomain, instance.id, true);
                    migrated.push(domain.clone());
                    config.domains.push(domain);
                }
            }
        }

        if !migrated.is_empty() {
            self.save(&config)?;
            // Ensure CA is trusted after migration
            let _ = crate::commands::auto_trust_ca_if_needed();
        }

        Ok(migrated)
    }

    // ========================================================================
    // Parked Directory Management
    // ========================================================================

    /// Get all parked directories
    pub fn list_parked_directories(&self) -> Result<Vec<ParkedDirectory>, String> {
        let config = self.load()?;
        Ok(config.parked_directories)
    }

    /// Get a specific parked directory by ID
    pub fn get_parked_directory(&self, id: Uuid) -> Result<ParkedDirectory, String> {
        let config = self.load()?;
        config
            .parked_directories
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Parked directory {} not found", id))
    }

    /// Create a new parked directory
    pub fn create_parked_directory(
        &self,
        path: String,
        ssl_enabled: bool,
    ) -> Result<ParkedDirectory, String> {
        let mut config = self.load()?;

        // Validate that the path exists and is a directory
        let path_buf = std::path::PathBuf::from(&path);
        if !path_buf.exists() {
            return Err(format!("Path '{}' does not exist", path));
        }
        if !path_buf.is_dir() {
            return Err(format!("Path '{}' is not a directory", path));
        }

        // Check for duplicate path
        if config.parked_directories.iter().any(|p| p.path == path) {
            return Err(format!("Directory '{}' is already parked", path));
        }

        let parked_dir = ParkedDirectory::new(path, ssl_enabled);

        config.parked_directories.push(parked_dir.clone());
        self.save(&config)?;

        Ok(parked_dir)
    }

    /// Delete a parked directory by ID
    pub fn delete_parked_directory(&self, id: Uuid) -> Result<(), String> {
        let mut config = self.load()?;

        let idx = config
            .parked_directories
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| format!("Parked directory {} not found", id))?;

        config.parked_directories.remove(idx);
        self.save(&config)?;

        Ok(())
    }

    /// Update SSL enabled status for a parked directory
    pub fn update_parked_directory_ssl(
        &self,
        id: Uuid,
        ssl_enabled: bool,
    ) -> Result<ParkedDirectory, String> {
        let mut config = self.load()?;

        let parked_dir = config
            .parked_directories
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Parked directory {} not found", id))?;

        parked_dir.ssl_enabled = ssl_enabled;

        let updated = parked_dir.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Get all domains associated with a parked directory
    pub fn get_domains_for_parked_directory(
        &self,
        parked_dir_id: Uuid,
    ) -> Result<Vec<Domain>, String> {
        let config = self.load()?;
        Ok(config
            .domains
            .into_iter()
            .filter(|d| d.parked_dir_id() == Some(parked_dir_id))
            .collect())
    }

    /// Delete all domains associated with a parked directory
    pub fn delete_domains_for_parked_directory(
        &self,
        parked_dir_id: Uuid,
    ) -> Result<Vec<Domain>, String> {
        let mut config = self.load()?;

        let (removed, remaining): (Vec<_>, Vec<_>) = config
            .domains
            .into_iter()
            .partition(|d| d.parked_dir_id() == Some(parked_dir_id));

        config.domains = remaining;
        self.save(&config)?;

        Ok(removed)
    }

    /// Check if a FrankenPHP Park instance exists (park feature is enabled)
    pub fn is_park_enabled(&self) -> Result<bool, String> {
        let config = self.load()?;
        Ok(config
            .instances
            .iter()
            .any(|i| i.service_type == ServiceType::FrankenPhpPark))
    }

    /// Get the FrankenPHP Park instance if it exists
    pub fn get_park_instance(&self) -> Result<Option<Instance>, String> {
        let config = self.load()?;
        Ok(config
            .instances
            .into_iter()
            .find(|i| i.service_type == ServiceType::FrankenPhpPark))
    }

    /// Find parked directory by path
    pub fn find_parked_directory_by_path(
        &self,
        path: &str,
    ) -> Result<Option<ParkedDirectory>, String> {
        let config = self.load()?;
        Ok(config
            .parked_directories
            .into_iter()
            .find(|p| p.path == path))
    }

    // ========================================================================
    // Stack Management
    // ========================================================================

    /// Get all stacks
    pub fn list_stacks(&self) -> Result<Vec<Stack>, String> {
        let config = self.load()?;
        Ok(config.stacks)
    }

    /// Get a specific stack by ID
    pub fn get_stack(&self, id: Uuid) -> Result<Stack, String> {
        let config = self.load()?;
        config
            .stacks
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Stack {} not found", id))
    }

    /// Create a new stack from selected instances
    pub fn create_stack(
        &self,
        name: String,
        description: Option<String>,
        instance_ids: Vec<Uuid>,
    ) -> Result<Stack, String> {
        let mut config = self.load()?;

        // Validate all instance IDs exist
        for instance_id in &instance_ids {
            if !config.instances.iter().any(|i| i.id == *instance_id) {
                return Err(format!("Instance {} not found", instance_id));
            }
        }

        let stack = Stack::new(name, description);

        // Update instances to point to this stack
        for instance in &mut config.instances {
            if instance_ids.contains(&instance.id) {
                instance.stack_id = Some(stack.id);
            }
        }

        config.stacks.push(stack.clone());
        self.save(&config)?;

        Ok(stack)
    }

    /// Update a stack's name and/or description
    pub fn update_stack(
        &self,
        id: Uuid,
        name: Option<String>,
        description: Option<Option<String>>,
    ) -> Result<Stack, String> {
        let mut config = self.load()?;

        let stack = config
            .stacks
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("Stack {} not found", id))?;

        if let Some(n) = name {
            stack.name = n;
        }
        if let Some(d) = description {
            stack.description = d;
        }
        stack.updated_at = Utc::now();

        let updated = stack.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Delete a stack
    /// If delete_instances is true, also deletes all instances in the stack
    /// If false, instances become standalone (stack_id = None)
    pub fn delete_stack(&self, id: Uuid, delete_instances: bool) -> Result<Vec<Uuid>, String> {
        let mut config = self.load()?;

        // Find the stack
        let stack_idx = config
            .stacks
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| format!("Stack {} not found", id))?;

        // Get instance IDs in this stack
        let instance_ids: Vec<Uuid> = config
            .instances
            .iter()
            .filter(|i| i.stack_id == Some(id))
            .map(|i| i.id)
            .collect();

        if delete_instances {
            // Remove instances that belong to this stack
            config.instances.retain(|i| i.stack_id != Some(id));
        } else {
            // Just unset the stack_id (move to standalone)
            for instance in &mut config.instances {
                if instance.stack_id == Some(id) {
                    instance.stack_id = None;
                }
            }
        }

        // Remove the stack
        config.stacks.remove(stack_idx);
        self.save(&config)?;

        Ok(instance_ids)
    }

    /// Add instances to a stack
    pub fn add_instances_to_stack(
        &self,
        stack_id: Uuid,
        instance_ids: Vec<Uuid>,
    ) -> Result<(), String> {
        let mut config = self.load()?;

        // Validate stack exists
        if !config.stacks.iter().any(|s| s.id == stack_id) {
            return Err(format!("Stack {} not found", stack_id));
        }

        // Validate all instance IDs exist
        for instance_id in &instance_ids {
            if !config.instances.iter().any(|i| i.id == *instance_id) {
                return Err(format!("Instance {} not found", instance_id));
            }
        }

        // Update instances
        for instance in &mut config.instances {
            if instance_ids.contains(&instance.id) {
                instance.stack_id = Some(stack_id);
            }
        }

        // Update stack's updated_at
        if let Some(stack) = config.stacks.iter_mut().find(|s| s.id == stack_id) {
            stack.updated_at = Utc::now();
        }

        self.save(&config)?;
        Ok(())
    }

    /// Remove instances from their stack (move to standalone)
    pub fn remove_instances_from_stack(&self, instance_ids: Vec<Uuid>) -> Result<(), String> {
        let mut config = self.load()?;

        // Track which stacks were affected
        let mut affected_stack_ids: Vec<Uuid> = Vec::new();

        for instance in &mut config.instances {
            if instance_ids.contains(&instance.id) {
                if let Some(stack_id) = instance.stack_id {
                    if !affected_stack_ids.contains(&stack_id) {
                        affected_stack_ids.push(stack_id);
                    }
                }
                instance.stack_id = None;
            }
        }

        // Update affected stacks' updated_at
        let now = Utc::now();
        for stack in &mut config.stacks {
            if affected_stack_ids.contains(&stack.id) {
                stack.updated_at = now;
            }
        }

        self.save(&config)?;
        Ok(())
    }

    /// Get all instances in a stack
    pub fn get_instances_in_stack(&self, stack_id: Uuid) -> Result<Vec<Instance>, String> {
        let config = self.load()?;
        Ok(config
            .instances
            .into_iter()
            .filter(|i| i.stack_id == Some(stack_id))
            .collect())
    }

    /// Get all standalone instances (not in any stack)
    pub fn get_standalone_instances(&self) -> Result<Vec<Instance>, String> {
        let config = self.load()?;
        Ok(config
            .instances
            .into_iter()
            .filter(|i| i.stack_id.is_none())
            .collect())
    }

    /// Reorder instances based on provided ID list
    pub fn reorder_instances(&self, instance_ids: Vec<Uuid>) -> Result<(), String> {
        let mut config = self.load()?;

        // Create a map of instance IDs to their desired position
        let position_map: std::collections::HashMap<Uuid, usize> = instance_ids
            .iter()
            .enumerate()
            .map(|(idx, id)| (*id, idx))
            .collect();

        // Sort instances: first by whether they're in the position map and their position,
        // then by existing order for instances not in the map
        config.instances.sort_by(
            |a, b| match (position_map.get(&a.id), position_map.get(&b.id)) {
                (Some(pos_a), Some(pos_b)) => pos_a.cmp(pos_b),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        );

        self.save(&config)?;
        Ok(())
    }

    /// Move an instance to a different stack (or to standalone if stack_id is None)
    pub fn move_instance_to_stack(
        &self,
        instance_id: Uuid,
        new_stack_id: Option<Uuid>,
    ) -> Result<Instance, String> {
        let mut config = self.load()?;

        // Validate new stack exists if provided
        if let Some(sid) = new_stack_id {
            if !config.stacks.iter().any(|s| s.id == sid) {
                return Err(format!("Stack {} not found", sid));
            }
        }

        // Track old and new stacks for updating timestamps
        let instance = config
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .ok_or_else(|| format!("Instance {} not found", instance_id))?;
        let old_stack_id = instance.stack_id;

        // Update instance
        let instance = config
            .instances
            .iter_mut()
            .find(|i| i.id == instance_id)
            .ok_or_else(|| format!("Instance {} not found", instance_id))?;

        instance.stack_id = new_stack_id;
        let updated = instance.clone();

        // Update timestamps on affected stacks
        let now = Utc::now();
        for stack in &mut config.stacks {
            if Some(stack.id) == old_stack_id || Some(stack.id) == new_stack_id {
                stack.updated_at = now;
            }
        }

        self.save(&config)?;
        Ok(updated)
    }

    // ========================================================================
    // frp Server Management
    // ========================================================================

    /// Get all frp servers
    pub fn list_frp_servers(&self) -> Result<Vec<FrpServer>, String> {
        let config = self.load()?;
        Ok(config.frp_servers)
    }

    /// Get a specific frp server by ID
    pub fn get_frp_server(&self, id: Uuid) -> Result<FrpServer, String> {
        let config = self.load()?;
        config
            .frp_servers
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| format!("frp server {} not found", id))
    }

    /// Get the default frp server
    pub fn get_default_frp_server(&self) -> Result<Option<FrpServer>, String> {
        let config = self.load()?;
        Ok(config.frp_servers.into_iter().find(|s| s.is_default))
    }

    /// Create a new frp server
    pub fn create_frp_server(
        &self,
        name: String,
        server_addr: String,
        server_port: u16,
        token: String,
        subdomain_host: String,
    ) -> Result<FrpServer, String> {
        let mut config = self.load()?;

        // If this is the first server, make it the default
        let is_first = config.frp_servers.is_empty();

        let server = FrpServer {
            id: Uuid::new_v4(),
            name,
            server_addr,
            server_port,
            token,
            subdomain_host,
            is_default: is_first,
            created_at: Utc::now(),
        };

        config.frp_servers.push(server.clone());
        self.save(&config)?;

        Ok(server)
    }

    /// Update an frp server
    #[allow(clippy::too_many_arguments)]
    pub fn update_frp_server(
        &self,
        id: Uuid,
        name: Option<String>,
        server_addr: Option<String>,
        server_port: Option<u16>,
        token: Option<String>,
        subdomain_host: Option<String>,
        is_default: Option<bool>,
    ) -> Result<FrpServer, String> {
        let mut config = self.load()?;

        // Find index first to avoid borrow checker issues
        let idx = config
            .frp_servers
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| format!("frp server {} not found", id))?;

        // Handle is_default first if needed (clear others)
        if let Some(true) = is_default {
            for s in &mut config.frp_servers {
                s.is_default = false;
            }
        }

        // Now update the server at the found index
        let server = &mut config.frp_servers[idx];

        if let Some(n) = name {
            server.name = n;
        }
        if let Some(a) = server_addr {
            server.server_addr = a;
        }
        if let Some(p) = server_port {
            server.server_port = p;
        }
        if let Some(t) = token {
            server.token = t;
        }
        if let Some(h) = subdomain_host {
            server.subdomain_host = h;
        }
        if let Some(d) = is_default {
            server.is_default = d;
        }

        let updated = server.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Delete an frp server
    pub fn delete_frp_server(&self, id: Uuid) -> Result<(), String> {
        let mut config = self.load()?;

        // Check if any tunnels use this server
        let tunnels_using_server = config.tunnels.iter().filter(|t| t.server_id == id).count();

        if tunnels_using_server > 0 {
            return Err(format!(
                "Cannot delete server: {} tunnel(s) are using it",
                tunnels_using_server
            ));
        }

        let idx = config
            .frp_servers
            .iter()
            .position(|s| s.id == id)
            .ok_or_else(|| format!("frp server {} not found", id))?;

        config.frp_servers.remove(idx);
        self.save(&config)?;

        Ok(())
    }

    // ========================================================================
    // Tunnel Management
    // ========================================================================

    /// Get all tunnels
    pub fn list_tunnels(&self) -> Result<Vec<Tunnel>, String> {
        let config = self.load()?;
        Ok(config.tunnels)
    }

    /// Get a specific tunnel by ID
    pub fn get_tunnel(&self, id: Uuid) -> Result<Tunnel, String> {
        let config = self.load()?;
        config
            .tunnels
            .into_iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Tunnel {} not found", id))
    }

    /// Create a new tunnel
    pub fn create_tunnel(
        &self,
        name: String,
        server_id: Uuid,
        target: TunnelTarget,
        subdomain: SubdomainConfig,
        protocol: String,
        auto_start: bool,
    ) -> Result<Tunnel, String> {
        let mut config = self.load()?;

        // Validate server exists
        if !config.frp_servers.iter().any(|s| s.id == server_id) {
            return Err(format!("frp server {} not found", server_id));
        }

        // Validate instance target if applicable
        if let TunnelTarget::Instance(instance_id) = &target {
            if !config.instances.iter().any(|i| i.id == *instance_id) {
                return Err(format!("Instance {} not found", instance_id));
            }
        }

        // Generate random subdomain immediately if needed
        let subdomain = match subdomain {
            SubdomainConfig::Random { generated: None } => SubdomainConfig::Random {
                generated: Some(crate::tunnel::generate_random_subdomain()),
            },
            other => other,
        };

        let tunnel = Tunnel {
            id: Uuid::new_v4(),
            name,
            server_id,
            target,
            subdomain,
            protocol,
            auto_start,
            created_at: Utc::now(),
        };

        config.tunnels.push(tunnel.clone());
        self.save(&config)?;

        Ok(tunnel)
    }

    /// Update a tunnel
    #[allow(clippy::too_many_arguments)]
    pub fn update_tunnel(
        &self,
        id: Uuid,
        name: Option<String>,
        server_id: Option<Uuid>,
        target: Option<TunnelTarget>,
        subdomain: Option<SubdomainConfig>,
        protocol: Option<String>,
        auto_start: Option<bool>,
    ) -> Result<Tunnel, String> {
        let mut config = self.load()?;

        // Validate server exists if changing
        if let Some(ref sid) = server_id {
            if !config.frp_servers.iter().any(|s| s.id == *sid) {
                return Err(format!("frp server {} not found", sid));
            }
        }

        // Validate instance target if changing
        if let Some(TunnelTarget::Instance(instance_id)) = &target {
            if !config.instances.iter().any(|i| i.id == *instance_id) {
                return Err(format!("Instance {} not found", instance_id));
            }
        }

        let tunnel = config
            .tunnels
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Tunnel {} not found", id))?;

        if let Some(n) = name {
            tunnel.name = n;
        }
        if let Some(sid) = server_id {
            tunnel.server_id = sid;
        }
        if let Some(t) = target {
            tunnel.target = t;
        }
        if let Some(s) = subdomain {
            tunnel.subdomain = s;
        }
        if let Some(p) = protocol {
            tunnel.protocol = p;
        }
        if let Some(a) = auto_start {
            tunnel.auto_start = a;
        }

        let updated = tunnel.clone();
        self.save(&config)?;

        Ok(updated)
    }

    /// Delete a tunnel
    pub fn delete_tunnel(&self, id: Uuid) -> Result<(), String> {
        let mut config = self.load()?;

        let idx = config
            .tunnels
            .iter()
            .position(|t| t.id == id)
            .ok_or_else(|| format!("Tunnel {} not found", id))?;

        config.tunnels.remove(idx);
        self.save(&config)?;

        Ok(())
    }

    /// Get tunnels for a specific server
    pub fn get_tunnels_for_server(&self, server_id: Uuid) -> Result<Vec<Tunnel>, String> {
        let config = self.load()?;
        Ok(config
            .tunnels
            .into_iter()
            .filter(|t| t.server_id == server_id)
            .collect())
    }

    /// Delete all tunnels that target a specific instance
    pub fn delete_tunnels_for_instance(&self, instance_id: Uuid) -> Result<Vec<Tunnel>, String> {
        let mut config = self.load()?;

        let (removed, remaining): (Vec<_>, Vec<_>) = config
            .tunnels
            .into_iter()
            .partition(|t| matches!(&t.target, TunnelTarget::Instance(id) if *id == instance_id));

        config.tunnels = remaining;
        self.save(&config)?;

        Ok(removed)
    }
}
