//! Test utilities and fixtures for unit and integration tests
//!
//! Provides builders, fixtures, and helper functions for testing Burd components.

use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

use crate::config::{Config, Domain, DomainTarget, Instance, ParkedDirectory, ServiceType, Stack};

// ============================================================================
// Instance Builders
// ============================================================================

/// Builder for creating test instances
pub struct InstanceBuilder {
    name: String,
    port: u16,
    service_type: ServiceType,
    version: String,
    config: serde_json::Value,
    auto_start: bool,
    domain: Option<String>,
    domain_enabled: bool,
    stack_id: Option<Uuid>,
}

impl InstanceBuilder {
    /// Create a new instance builder with sensible defaults
    pub fn new() -> Self {
        Self {
            name: "test-instance".to_string(),
            port: 7700,
            service_type: ServiceType::Meilisearch,
            version: "1.6.0".to_string(),
            config: json!({}),
            auto_start: false,
            domain: None,
            domain_enabled: true,
            stack_id: None,
        }
    }

    /// Set the instance name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the port
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the service type
    pub fn service_type(mut self, service_type: ServiceType) -> Self {
        self.service_type = service_type;
        self
    }

    /// Set the version
    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set the service config
    pub fn config(mut self, config: serde_json::Value) -> Self {
        self.config = config;
        self
    }

    /// Set auto_start
    pub fn auto_start(mut self, auto_start: bool) -> Self {
        self.auto_start = auto_start;
        self
    }

    /// Set the custom domain
    pub fn domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = Some(domain.into());
        self
    }

    /// Set domain_enabled
    pub fn domain_enabled(mut self, enabled: bool) -> Self {
        self.domain_enabled = enabled;
        self
    }

    /// Set the stack_id
    pub fn stack_id(mut self, stack_id: Uuid) -> Self {
        self.stack_id = Some(stack_id);
        self
    }

    /// Build the instance
    pub fn build(self) -> Instance {
        Instance {
            id: Uuid::new_v4(),
            name: self.name,
            port: self.port,
            service_type: self.service_type,
            version: self.version,
            config: self.config,
            master_key: None,
            auto_start: self.auto_start,
            created_at: Utc::now(),
            domain: self.domain,
            domain_enabled: self.domain_enabled,
            stack_id: self.stack_id,
        }
    }

    /// Build the instance with a specific ID
    pub fn build_with_id(self, id: Uuid) -> Instance {
        let mut instance = self.build();
        instance.id = id;
        instance
    }
}

impl Default for InstanceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Domain Builders
// ============================================================================

/// Builder for creating test domains
pub struct DomainBuilder {
    subdomain: String,
    target: DomainTarget,
    ssl_enabled: bool,
}

impl DomainBuilder {
    /// Create a new domain builder with instance target
    pub fn new_instance(instance_id: Uuid) -> Self {
        Self {
            subdomain: "test".to_string(),
            target: DomainTarget::Instance(instance_id),
            ssl_enabled: false,
        }
    }

    /// Create a new domain builder with port target
    pub fn new_port(port: u16) -> Self {
        Self {
            subdomain: "test".to_string(),
            target: DomainTarget::Port(port),
            ssl_enabled: false,
        }
    }

    /// Create a new domain builder with static files target
    pub fn new_static(path: impl Into<String>) -> Self {
        Self {
            subdomain: "test".to_string(),
            target: DomainTarget::StaticFiles {
                path: path.into(),
                browse: false,
            },
            ssl_enabled: false,
        }
    }

    /// Set the subdomain
    pub fn subdomain(mut self, subdomain: impl Into<String>) -> Self {
        self.subdomain = subdomain.into();
        self
    }

    /// Enable SSL
    pub fn ssl_enabled(mut self, enabled: bool) -> Self {
        self.ssl_enabled = enabled;
        self
    }

    /// Build the domain
    pub fn build(self) -> Domain {
        match self.target {
            DomainTarget::Instance(id) => {
                Domain::for_instance(self.subdomain, id, self.ssl_enabled)
            }
            DomainTarget::Port(port) => Domain::for_port(self.subdomain, port, self.ssl_enabled),
            DomainTarget::StaticFiles { path, browse } => {
                Domain::for_static_files(self.subdomain, path, browse, self.ssl_enabled)
            }
        }
    }

    /// Build the domain with a specific ID
    pub fn build_with_id(self, id: Uuid) -> Domain {
        let mut domain = self.build();
        domain.id = id;
        domain
    }
}

// ============================================================================
// Stack Builders
// ============================================================================

/// Builder for creating test stacks
pub struct StackBuilder {
    name: String,
    description: Option<String>,
}

impl StackBuilder {
    /// Create a new stack builder
    pub fn new() -> Self {
        Self {
            name: "test-stack".to_string(),
            description: None,
        }
    }

    /// Set the stack name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the description
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Build the stack
    pub fn build(self) -> Stack {
        Stack::new(self.name, self.description)
    }

    /// Build the stack with a specific ID
    pub fn build_with_id(self, id: Uuid) -> Stack {
        let mut stack = self.build();
        stack.id = id;
        stack
    }
}

impl Default for StackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Config Fixtures
// ============================================================================

/// Create a temporary config directory for testing
pub struct TempConfigDir {
    #[allow(dead_code)]
    temp_dir: TempDir,
    pub config_path: PathBuf,
}

impl TempConfigDir {
    /// Create a new temporary config directory
    pub fn new() -> Result<Self, String> {
        let temp_dir = TempDir::new().map_err(|e| format!("Failed to create temp dir: {}", e))?;

        let config_path = temp_dir.path().join("config.json");

        Ok(Self {
            temp_dir,
            config_path,
        })
    }

    /// Get the path to the temp directory
    pub fn path(&self) -> &std::path::Path {
        self.temp_dir.path()
    }

    /// Write a config to the temp directory
    pub fn write_config(&self, config: &Config) -> Result<(), String> {
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(&self.config_path, content)
            .map_err(|e| format!("Failed to write config: {}", e))
    }

    /// Read the config from the temp directory
    pub fn read_config(&self) -> Result<Config, String> {
        let content = std::fs::read_to_string(&self.config_path)
            .map_err(|e| format!("Failed to read config: {}", e))?;

        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))
    }
}

// ============================================================================
// Config Builder
// ============================================================================

/// Builder for creating test configs
pub struct ConfigBuilder {
    instances: Vec<Instance>,
    domains: Vec<Domain>,
    stacks: Vec<Stack>,
    parked_directories: Vec<ParkedDirectory>,
    dns_port: u16,
    proxy_port: u16,
    tld: String,
}

impl ConfigBuilder {
    /// Create a new config builder
    pub fn new() -> Self {
        Self {
            instances: Vec::new(),
            domains: Vec::new(),
            stacks: Vec::new(),
            parked_directories: Vec::new(),
            dns_port: 5300,
            proxy_port: 8080,
            tld: "test".to_string(),
        }
    }

    /// Add an instance
    pub fn instance(mut self, instance: Instance) -> Self {
        self.instances.push(instance);
        self
    }

    /// Add multiple instances
    pub fn instances(mut self, instances: Vec<Instance>) -> Self {
        self.instances.extend(instances);
        self
    }

    /// Add a domain
    pub fn domain(mut self, domain: Domain) -> Self {
        self.domains.push(domain);
        self
    }

    /// Add multiple domains
    pub fn domains(mut self, domains: Vec<Domain>) -> Self {
        self.domains.extend(domains);
        self
    }

    /// Add a stack
    pub fn stack(mut self, stack: Stack) -> Self {
        self.stacks.push(stack);
        self
    }

    /// Set the DNS port
    pub fn dns_port(mut self, port: u16) -> Self {
        self.dns_port = port;
        self
    }

    /// Set the proxy port
    pub fn proxy_port(mut self, port: u16) -> Self {
        self.proxy_port = port;
        self
    }

    /// Set the TLD
    pub fn tld(mut self, tld: impl Into<String>) -> Self {
        self.tld = tld.into();
        self
    }

    /// Build the config
    pub fn build(self) -> Config {
        Config {
            instances: self.instances,
            domains: self.domains,
            stacks: self.stacks,
            parked_directories: self.parked_directories,
            binaries: std::collections::HashMap::new(),
            dns_port: self.dns_port,
            proxy_port: self.proxy_port,
            tld: self.tld,
            proxy_installed: false,
            frp_servers: Vec::new(),
            tunnels: Vec::new(),
        }
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Service Type Fixtures
// ============================================================================

/// Get all supported service types for testing
pub fn all_service_types() -> Vec<ServiceType> {
    ServiceType::all()
}

/// Get database service types
pub fn database_service_types() -> Vec<ServiceType> {
    vec![
        ServiceType::MySQL,
        ServiceType::MariaDB,
        ServiceType::PostgreSQL,
        ServiceType::MongoDB,
    ]
}

/// Get key-value store service types
pub fn key_value_service_types() -> Vec<ServiceType> {
    vec![ServiceType::Redis, ServiceType::Valkey]
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a test instance with minimal configuration
pub fn test_instance(name: &str, port: u16, service_type: ServiceType) -> Instance {
    InstanceBuilder::new()
        .name(name)
        .port(port)
        .service_type(service_type)
        .build()
}

/// Create a test domain routing to an instance
pub fn test_domain_for_instance(subdomain: &str, instance_id: Uuid) -> Domain {
    DomainBuilder::new_instance(instance_id)
        .subdomain(subdomain)
        .build()
}

/// Create a test domain routing to a port
pub fn test_domain_for_port(subdomain: &str, port: u16) -> Domain {
    DomainBuilder::new_port(port).subdomain(subdomain).build()
}

/// Create a test stack
pub fn test_stack(name: &str) -> Stack {
    StackBuilder::new().name(name).build()
}

/// Create a test config with some instances
pub fn test_config_with_instances(instances: Vec<Instance>) -> Config {
    ConfigBuilder::new().instances(instances).build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instance_builder() {
        let instance = InstanceBuilder::new()
            .name("my-service")
            .port(8080)
            .service_type(ServiceType::Redis)
            .version("7.0.0")
            .auto_start(true)
            .build();

        assert_eq!(instance.name, "my-service");
        assert_eq!(instance.port, 8080);
        assert_eq!(instance.service_type, ServiceType::Redis);
        assert_eq!(instance.version, "7.0.0");
        assert!(instance.auto_start);
    }

    #[test]
    fn test_domain_builder_instance_target() {
        let instance_id = Uuid::new_v4();
        let domain = DomainBuilder::new_instance(instance_id)
            .subdomain("api")
            .ssl_enabled(true)
            .build();

        assert_eq!(domain.subdomain, "api");
        assert!(domain.ssl_enabled);
        assert!(matches!(domain.target, DomainTarget::Instance(id) if id == instance_id));
    }

    #[test]
    fn test_config_builder() {
        let instance = test_instance("test", 7700, ServiceType::Meilisearch);
        let config = ConfigBuilder::new()
            .instance(instance.clone())
            .dns_port(5300)
            .tld("dev")
            .build();

        assert_eq!(config.instances.len(), 1);
        assert_eq!(config.instances[0].name, "test");
        assert_eq!(config.dns_port, 5300);
        assert_eq!(config.tld, "dev");
    }

    #[test]
    fn test_temp_config_dir() {
        let temp_dir = TempConfigDir::new().unwrap();
        let config = test_config_with_instances(vec![]);

        temp_dir.write_config(&config).unwrap();
        let loaded = temp_dir.read_config().unwrap();

        assert_eq!(loaded.instances.len(), 0);
        assert_eq!(loaded.tld, config.tld);
    }

    #[test]
    fn test_service_type_groupings() {
        let all = all_service_types();
        let databases = database_service_types();
        let kv_stores = key_value_service_types();

        assert!(all.len() > databases.len());
        assert!(databases.contains(&ServiceType::MySQL));
        assert!(kv_stores.contains(&ServiceType::Redis));
    }
}
