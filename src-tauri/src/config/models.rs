//! Configuration data models
//!
//! Contains Domain, Instance, ServiceType, BinaryInfo, and Config structs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// Re-export tunnel types for convenience
pub use crate::tunnel::{FrpServer, Tunnel, TunnelTarget, SubdomainConfig, TunnelState, TunnelWithState};

// ============================================================================
// Domain Entity
// ============================================================================

/// Target for a domain - instance, port, or static file server
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum DomainTarget {
    /// Route to a Burd instance by its ID
    Instance(Uuid),
    /// Route to a raw port (for non-Burd services)
    Port(u16),
    /// Serve static files from a directory
    StaticFiles {
        /// Root directory path to serve files from
        path: String,
        /// Enable directory listing when no index file exists
        browse: bool,
    },
}

/// Tracks where a domain originated from
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type")]
pub enum DomainSource {
    /// Manually created domain
    #[default]
    Manual,
    /// Auto-created from a parked directory
    Parked {
        /// The parked directory this domain belongs to
        parked_dir_id: Uuid,
    },
    /// Was parked but has been isolated to its own instance
    Isolated {
        /// The original parked directory
        original_parked_dir_id: Uuid,
    },
}

/// A domain mapping that routes subdomain.tld to a target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Domain {
    pub id: Uuid,
    /// Subdomain only (e.g., "api" not "api.burd")
    pub subdomain: String,
    /// What this domain routes to
    pub target: DomainTarget,
    /// Whether SSL (HTTPS) is enabled for this domain
    #[serde(default)]
    pub ssl_enabled: bool,
    /// Where this domain originated from (manual, parked, or isolated)
    #[serde(default)]
    pub source: DomainSource,
    /// When this domain was created
    pub created_at: DateTime<Utc>,
}

impl Domain {
    /// Create a new domain routing to an instance
    pub fn for_instance(subdomain: String, instance_id: Uuid, ssl_enabled: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            subdomain,
            target: DomainTarget::Instance(instance_id),
            ssl_enabled,
            source: DomainSource::Manual,
            created_at: Utc::now(),
        }
    }

    /// Create a new domain routing to a raw port
    pub fn for_port(subdomain: String, port: u16, ssl_enabled: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            subdomain,
            target: DomainTarget::Port(port),
            ssl_enabled,
            source: DomainSource::Manual,
            created_at: Utc::now(),
        }
    }

    /// Create a new domain serving static files from a directory
    pub fn for_static_files(subdomain: String, path: String, browse: bool, ssl_enabled: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            subdomain,
            target: DomainTarget::StaticFiles { path, browse },
            ssl_enabled,
            source: DomainSource::Manual,
            created_at: Utc::now(),
        }
    }

    /// Create a parked domain routing to the park server port
    pub fn for_parked_port(subdomain: String, port: u16, ssl_enabled: bool, parked_dir_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            subdomain,
            target: DomainTarget::Port(port),
            ssl_enabled,
            source: DomainSource::Parked { parked_dir_id },
            created_at: Utc::now(),
        }
    }

    /// Create a parked domain serving static files
    pub fn for_parked_static_files(subdomain: String, path: String, browse: bool, ssl_enabled: bool, parked_dir_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            subdomain,
            target: DomainTarget::StaticFiles { path, browse },
            ssl_enabled,
            source: DomainSource::Parked { parked_dir_id },
            created_at: Utc::now(),
        }
    }

    /// Check if this domain is from a parked directory
    pub fn is_parked(&self) -> bool {
        matches!(self.source, DomainSource::Parked { .. })
    }

    /// Check if this domain is isolated (was parked, now has its own instance)
    pub fn is_isolated(&self) -> bool {
        matches!(self.source, DomainSource::Isolated { .. })
    }

    /// Get the parked directory ID if this is a parked or isolated domain
    pub fn parked_dir_id(&self) -> Option<Uuid> {
        match &self.source {
            DomainSource::Parked { parked_dir_id } => Some(*parked_dir_id),
            DomainSource::Isolated { original_parked_dir_id } => Some(*original_parked_dir_id),
            DomainSource::Manual => None,
        }
    }

    /// Get the full domain with TLD (e.g., "api.burd")
    pub fn full_domain(&self, tld: &str) -> String {
        format!("{}.{}", self.subdomain, tld)
    }

    /// Get the target port (resolves instance to its port if needed)
    /// Returns None for StaticFiles targets since they don't proxy to a port
    pub fn get_target_port(&self, instances: &[Instance]) -> Option<u16> {
        match &self.target {
            DomainTarget::Port(port) => Some(*port),
            DomainTarget::Instance(instance_id) => {
                instances.iter()
                    .find(|i| i.id == *instance_id)
                    .map(|i| i.port)
            }
            DomainTarget::StaticFiles { .. } => None, // Static files don't use a port
        }
    }

    /// Check if this domain routes to a specific instance
    pub fn routes_to_instance(&self, instance_id: &Uuid) -> bool {
        matches!(&self.target, DomainTarget::Instance(id) if id == instance_id)
    }
}

// ============================================================================
// Service Type
// ============================================================================

/// Supported service types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    Meilisearch,
    MongoDB,
    Typesense,
    MinIO,
    FrankenPHP,
    #[serde(rename = "frankenphp-park")]
    FrankenPhpPark,
    MariaDB,
    MySQL,
    PostgreSQL,
    Redis,
    Valkey,
    Mailpit,
    Beanstalkd,
    Memcached,
    Frpc,
    NodeRed,
    Caddy,
    Centrifugo,
    Gitea,
}

impl ServiceType {
    pub fn display_name(&self) -> &'static str {
        match self {
            ServiceType::Meilisearch => "Meilisearch",
            ServiceType::MongoDB => "MongoDB",
            ServiceType::Typesense => "Typesense",
            ServiceType::MinIO => "MinIO",
            ServiceType::FrankenPHP => "PHP",
            ServiceType::FrankenPhpPark => "PHP Park",
            ServiceType::MariaDB => "MariaDB",
            ServiceType::MySQL => "MySQL",
            ServiceType::PostgreSQL => "PostgreSQL",
            ServiceType::Redis => "Redis",
            ServiceType::Valkey => "Valkey",
            ServiceType::Mailpit => "Mailpit",
            ServiceType::Beanstalkd => "Beanstalkd",
            ServiceType::Memcached => "Memcached",
            ServiceType::Frpc => "Tunnels (frpc)",
            ServiceType::NodeRed => "Node-RED",
            ServiceType::Caddy => "Caddy",
            ServiceType::Centrifugo => "Centrifugo",
            ServiceType::Gitea => "Gitea",
        }
    }

    /// Get the service ID as a string (lowercase, matches JSON config keys)
    pub fn as_str(&self) -> &'static str {
        match self {
            ServiceType::Meilisearch => "meilisearch",
            ServiceType::MongoDB => "mongodb",
            ServiceType::Typesense => "typesense",
            ServiceType::MinIO => "minio",
            ServiceType::FrankenPHP => "frankenphp",
            ServiceType::FrankenPhpPark => "frankenphp-park",
            ServiceType::MariaDB => "mariadb",
            ServiceType::MySQL => "mysql",
            ServiceType::PostgreSQL => "postgresql",
            ServiceType::Redis => "redis",
            ServiceType::Valkey => "valkey",
            ServiceType::Mailpit => "mailpit",
            ServiceType::Beanstalkd => "beanstalkd",
            ServiceType::Memcached => "memcached",
            ServiceType::Frpc => "frpc",
            ServiceType::NodeRed => "nodered",
            ServiceType::Caddy => "caddy",
            ServiceType::Centrifugo => "centrifugo",
            ServiceType::Gitea => "gitea",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            ServiceType::Meilisearch => 7700,
            ServiceType::MongoDB => 27017,
            ServiceType::Typesense => 8108,
            ServiceType::MinIO => 9000,
            ServiceType::FrankenPHP => 8000,
            ServiceType::FrankenPhpPark => 8888,
            ServiceType::MariaDB => 3330,
            ServiceType::MySQL => 3306,
            ServiceType::PostgreSQL => 5432,
            ServiceType::Redis => 6379,
            ServiceType::Valkey => 6380,
            ServiceType::Mailpit => 8025,
            ServiceType::Beanstalkd => 11300,
            ServiceType::Memcached => 11211,
            ServiceType::Frpc => 0, // frpc doesn't have a default port
            ServiceType::NodeRed => 1880,
            ServiceType::Caddy => 443,
            ServiceType::Centrifugo => 8000,
            ServiceType::Gitea => 3000,
        }
    }

    pub fn all() -> Vec<ServiceType> {
        vec![
            ServiceType::Meilisearch,
            ServiceType::MongoDB,
            ServiceType::Typesense,
            ServiceType::MinIO,
            ServiceType::FrankenPHP,
            ServiceType::FrankenPhpPark,
            ServiceType::MariaDB,
            ServiceType::MySQL,
            ServiceType::PostgreSQL,
            ServiceType::Redis,
            ServiceType::Valkey,
            ServiceType::Mailpit,
            ServiceType::Beanstalkd,
            ServiceType::Memcached,
            ServiceType::Frpc,
            ServiceType::NodeRed,
            ServiceType::Centrifugo,
            ServiceType::Gitea,
        ]
    }
}

// ============================================================================
// Instance Entity
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: Uuid,
    pub name: String,
    pub port: u16,
    #[serde(default = "default_service_type")]
    pub service_type: ServiceType,
    /// Binary version this instance uses (e.g., "1.6.0", "8.0.4", "system")
    #[serde(default)]
    pub version: String,
    /// Service-specific configuration (e.g., master_key for Meilisearch)
    #[serde(default)]
    pub config: serde_json::Value,
    /// Legacy field - migrated to config.master_key
    #[serde(default, skip_serializing)]
    pub master_key: Option<String>,
    #[serde(default)]
    pub auto_start: bool,
    pub created_at: DateTime<Utc>,
    /// Custom domain override (without TLD, e.g., "my-api" instead of "my-api.jonny")
    #[serde(default)]
    pub domain: Option<String>,
    /// Whether domain routing is enabled for this instance
    #[serde(default = "default_domain_enabled")]
    pub domain_enabled: bool,
    /// Stack this instance belongs to (None = standalone)
    #[serde(default)]
    pub stack_id: Option<Uuid>,
}

fn default_domain_enabled() -> bool {
    true
}

fn default_service_type() -> ServiceType {
    ServiceType::Meilisearch
}

impl Instance {
    /// Get master_key from config (for Meilisearch compatibility)
    /// Also checks legacy master_key field for migration
    pub fn get_master_key(&self) -> Option<String> {
        // First check new config location
        if let Some(key) = self.config.get("master_key").and_then(|v| v.as_str()) {
            if !key.is_empty() {
                return Some(key.to_string());
            }
        }
        // Fall back to legacy field
        self.master_key.clone().filter(|k| !k.is_empty())
    }

    /// Migrate legacy fields to new config structure
    pub fn migrate(&mut self) {
        // Migrate master_key to config
        if let Some(ref key) = self.master_key {
            if !key.is_empty() && self.config.get("master_key").is_none() {
                if let serde_json::Value::Object(ref mut map) = self.config {
                    map.insert("master_key".to_string(), serde_json::Value::String(key.clone()));
                } else {
                    self.config = serde_json::json!({ "master_key": key });
                }
            }
        }
        self.master_key = None;
    }

    /// Generate a domain slug from the instance name
    pub fn generate_domain_slug(&self) -> String {
        slug::slugify(&self.name)
    }

    /// Get the effective domain slug (custom or auto-generated)
    pub fn effective_domain_slug(&self) -> String {
        self.domain.clone().unwrap_or_else(|| self.generate_domain_slug())
    }

    /// Get the full domain with TLD (e.g., "my-api.burd")
    pub fn full_domain(&self, tld: &str) -> String {
        format!("{}.{}", self.effective_domain_slug(), tld)
    }
}

// ============================================================================
// Binary Info
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryInfo {
    pub version: String,
    pub path: String,
    pub downloaded_at: DateTime<Utc>,
}

/// Custom deserializer to migrate from old single-version format to multi-version format
/// Old format: { "meilisearch": BinaryInfo }
/// New format: { "meilisearch": { "1.6.0": BinaryInfo } }
pub fn deserialize_binaries<'de, D>(
    deserializer: D,
) -> Result<HashMap<ServiceType, HashMap<String, BinaryInfo>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    use serde_json::Value;

    let value = Value::deserialize(deserializer)?;

    match value {
        Value::Object(map) => {
            let mut result: HashMap<ServiceType, HashMap<String, BinaryInfo>> = HashMap::new();

            for (key, val) in map {
                // Parse service type
                let service_type: ServiceType = serde_json::from_value(Value::String(key.clone()))
                    .map_err(|e| D::Error::custom(format!("Invalid service type '{}': {}", key, e)))?;

                // Check if it's old format (direct BinaryInfo) or new format (version map)
                if val.get("version").is_some() && val.get("path").is_some() {
                    // Old format: migrate to new format
                    let binary_info: BinaryInfo = serde_json::from_value(val)
                        .map_err(|e| D::Error::custom(format!("Invalid binary info: {}", e)))?;
                    let version = binary_info.version.clone();
                    let mut version_map = HashMap::new();
                    version_map.insert(version, binary_info);
                    result.insert(service_type, version_map);
                } else if let Value::Object(version_map) = val {
                    // New format: version -> BinaryInfo mapping
                    let mut versions = HashMap::new();
                    for (version, info) in version_map {
                        let binary_info: BinaryInfo = serde_json::from_value(info)
                            .map_err(|e| D::Error::custom(format!("Invalid binary info for version '{}': {}", version, e)))?;
                        versions.insert(version, binary_info);
                    }
                    result.insert(service_type, versions);
                }
            }

            Ok(result)
        }
        Value::Null => Ok(HashMap::new()),
        _ => Err(D::Error::custom("Expected object for binaries")),
    }
}

// ============================================================================
// Parked Directory Entity
// ============================================================================

/// A parked directory where subdirectories automatically become domains
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParkedDirectory {
    pub id: Uuid,
    /// Absolute path to the parked directory (e.g., "/Users/dev/Sites")
    pub path: String,
    /// Whether SSL (HTTPS) is enabled for parked domains from this directory
    #[serde(default)]
    pub ssl_enabled: bool,
    /// When this directory was parked
    pub created_at: DateTime<Utc>,
}

impl ParkedDirectory {
    /// Create a new parked directory
    pub fn new(path: String, ssl_enabled: bool) -> Self {
        Self {
            id: Uuid::new_v4(),
            path,
            ssl_enabled,
            created_at: Utc::now(),
        }
    }
}

// ============================================================================
// Stack Entity
// ============================================================================

/// A stack groups related instances together for team sharing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stack {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Stack {
    /// Create a new stack
    pub fn new(name: String, description: Option<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            created_at: now,
            updated_at: now,
        }
    }
}

// ============================================================================
// Stack Export Format (for sharing)
// ============================================================================

/// Export format for sharing stacks between team members
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackExport {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    /// Schema version for future format migrations
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Who created/exported this config
    #[serde(default)]
    pub created_by: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub services: Vec<StackService>,
    #[serde(default)]
    pub domains: Vec<StackDomain>,
    #[serde(default)]
    pub requirements: StackRequirements,
}

fn default_schema_version() -> u32 {
    1
}

/// A service definition within a stack export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackService {
    /// Internal reference ID for linking domains to this service
    pub ref_id: String,
    pub service_type: ServiceType,
    pub version: String,
    pub name: String,
    pub port: u16,
    #[serde(default)]
    pub auto_start: bool,
    /// Service-specific config (secrets should be stripped)
    #[serde(default)]
    pub config: serde_json::Value,
}

/// A domain definition within a stack export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackDomain {
    pub subdomain: String,
    /// References a StackService by its ref_id
    pub target_ref: String,
    #[serde(default)]
    pub ssl_enabled: bool,
}

/// Requirements for importing a stack
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StackRequirements {
    /// Minimum Burd version required
    #[serde(default)]
    pub min_burd_version: Option<String>,
}

// ============================================================================
// Stack Import Types
// ============================================================================

/// Preview result when validating a stack import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackImportPreview {
    pub config: StackExport,
    pub missing_versions: Vec<MissingVersion>,
    pub conflicts: Vec<ImportConflict>,
    /// If a stack with this ID already exists
    pub existing_stack: Option<Stack>,
}

/// A service version that needs to be downloaded
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingVersion {
    pub service_type: ServiceType,
    pub version: String,
    #[serde(default)]
    pub download_size: Option<u64>,
}

/// Conflicts detected during import
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImportConflict {
    PortInUse {
        port: u16,
        existing_instance_name: String,
        new_service_ref: String,
    },
    NameExists {
        name: String,
        existing_id: Uuid,
        new_service_ref: String,
    },
    StackIdExists {
        existing_stack_name: String,
    },
}

/// How to resolve a specific conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConflictResolution {
    ReassignPort {
        service_ref: String,
        new_port: u16,
    },
    RenameService {
        service_ref: String,
        new_name: String,
    },
    ReplaceExisting {
        service_ref: String,
    },
    Skip {
        service_ref: String,
    },
    /// Update the existing stack with the imported config
    UpdateExistingStack,
}

/// Result of a successful import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub stack: Stack,
    pub instances_created: Vec<Uuid>,
    pub instances_updated: Vec<Uuid>,
    pub instances_skipped: Vec<String>,
    pub domains_created: Vec<Uuid>,
}

// ============================================================================
// Config
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub instances: Vec<Instance>,
    /// Domain mappings (separate from instances)
    #[serde(default)]
    pub domains: Vec<Domain>,
    /// Stacks for grouping related instances
    #[serde(default)]
    pub stacks: Vec<Stack>,
    /// Parked directories for automatic domain creation
    #[serde(default)]
    pub parked_directories: Vec<ParkedDirectory>,
    /// Binaries per service type - supports multiple versions
    /// Structure: { ServiceType: { "version": BinaryInfo } }
    #[serde(default, deserialize_with = "deserialize_binaries")]
    pub binaries: HashMap<ServiceType, HashMap<String, BinaryInfo>>,
    /// DNS server port
    #[serde(default = "default_dns_port")]
    pub dns_port: u16,
    /// Proxy server port (unprivileged fallback)
    #[serde(default = "default_proxy_port")]
    pub proxy_port: u16,
    /// Custom TLD for domain routing (e.g., "burd" for .burd domains)
    #[serde(default = "default_tld")]
    pub tld: String,
    /// Whether the privileged proxy daemon is installed (launchd on macOS)
    /// When true, the proxy runs on ports 80/443 via system daemon
    #[serde(default)]
    pub proxy_installed: bool,
    /// frp server configurations for tunneling
    #[serde(default)]
    pub frp_servers: Vec<FrpServer>,
    /// Tunnel configurations
    #[serde(default)]
    pub tunnels: Vec<Tunnel>,
}

fn default_dns_port() -> u16 {
    crate::domain::DEFAULT_DNS_PORT
}

fn default_proxy_port() -> u16 {
    crate::domain::DEFAULT_PROXY_PORT
}

fn default_tld() -> String {
    crate::domain::DEFAULT_TLD.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            instances: Vec::new(),
            domains: Vec::new(),
            stacks: Vec::new(),
            parked_directories: Vec::new(),
            binaries: HashMap::new(),
            dns_port: default_dns_port(),
            proxy_port: default_proxy_port(),
            tld: default_tld(),
            proxy_installed: false,
            frp_servers: Vec::new(),
            tunnels: Vec::new(),
        }
    }
}
