pub mod beanstalkd;
pub mod centrifugo;
pub mod frankenphp;
pub mod frankenphp_park;
pub mod frpc;
pub mod gitea;
pub mod key_value_service;
pub mod mailpit;
pub mod mariadb;
pub mod meilisearch;
pub mod mysql;
pub mod memcached;
pub mod minio;
pub mod mongodb;
pub mod nodered;
pub mod postgresql;
pub mod redis;
pub mod typesense;
pub mod valkey;

use crate::config::{Instance, ServiceType};
use std::path::Path;

/// Health check method for a service
#[derive(Debug, Clone)]
pub enum HealthCheck {
    /// HTTP GET request to a path
    Http { path: String },
    /// TCP connection test
    Tcp,
}

/// Download method for a service
#[derive(Debug, Clone)]
pub enum DownloadMethod {
    /// Use GitHub releases API to find assets
    GitHubRelease {
        api_url: &'static str,
        asset_pattern: String,
        /// Optional SHA256 checksum for verification
        checksum: Option<&'static str>,
    },
    /// Direct download URL
    Direct {
        url: String,
        /// If true, the download is a tar.gz that needs extraction
        is_archive: bool,
        /// Optional SHA256 checksum for verification
        checksum: Option<&'static str>,
    },
}

/// How to fetch available versions for a service
#[derive(Debug, Clone)]
pub enum VersionSource {
    /// Fetch from GitHub releases API
    GitHubReleases(&'static str),
    /// Hardcoded list of known versions
    Static(Vec<&'static str>),
}

/// Process manager type for a service
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessManager {
    /// Direct process spawning (default for most services)
    Binary,
    /// Node.js PM2 process manager
    Pm2,
    // Future: Docker, Systemd, etc.
}

/// Trait defining a service's behavior
#[allow(dead_code)]
pub trait ServiceDefinition: Send + Sync {
    /// Get the service type
    fn service_type(&self) -> ServiceType;

    /// Display name for the UI
    fn display_name(&self) -> &'static str;

    /// Default port for this service
    fn default_port(&self) -> u16;

    /// Binary filename
    fn binary_name(&self) -> &'static str;

    /// Get the source for fetching available versions
    fn version_source(&self) -> VersionSource;

    /// Get the download method for a specific version
    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod;

    /// Health check configuration
    fn health_check(&self) -> HealthCheck;

    /// Build command line arguments for starting the service
    fn start_args(&self, instance: &Instance, data_dir: &Path) -> Vec<String>;

    /// Environment variables to set when starting the service
    /// The `domain` parameter contains the full domain (e.g., "my-app.burd") if domain routing is enabled
    fn env_vars(&self, instance: &Instance, domain: Option<&str>) -> Vec<(String, String)> {
        let _ = instance;
        let _ = domain;
        vec![]
    }

    /// Whether this service needs initialization before first start
    fn needs_init(&self) -> bool {
        false
    }

    /// Initialize the service (e.g., initdb for postgres)
    fn init_command(&self, _data_dir: &Path) -> Option<(String, Vec<String>)> {
        None
    }

    /// Get the process manager type for this service
    fn process_manager(&self) -> ProcessManager {
        ProcessManager::Binary
    }
}

/// Get the service definition for a given service type
pub fn get_service(service_type: ServiceType) -> Box<dyn ServiceDefinition> {
    match service_type {
        ServiceType::Meilisearch => Box::new(meilisearch::MeilisearchService),
        ServiceType::MongoDB => Box::new(mongodb::MongoDBService),
        ServiceType::Typesense => Box::new(typesense::TypesenseService),
        ServiceType::MinIO => Box::new(minio::MinIOService),
        ServiceType::FrankenPHP => Box::new(frankenphp::FrankenPHPService),
        ServiceType::FrankenPhpPark => Box::new(frankenphp_park::FrankenPHPParkService),
        ServiceType::MariaDB => Box::new(mariadb::MariaDBService),
        ServiceType::MySQL => Box::new(mysql::MySQLService),
        ServiceType::PostgreSQL => Box::new(postgresql::PostgreSQLService),
        ServiceType::Redis => Box::new(redis::RedisService::new()),
        ServiceType::Valkey => Box::new(valkey::ValkeyService::new()),
        ServiceType::Mailpit => Box::new(mailpit::MailpitService),
        ServiceType::Beanstalkd => Box::new(beanstalkd::BeanstalkdService),
        ServiceType::Memcached => Box::new(memcached::MemcachedService),
        ServiceType::Frpc => Box::new(frpc::FrpcService),
        ServiceType::NodeRed => Box::new(nodered::NodeRedService),
        ServiceType::Caddy => panic!("Caddy is an internal service and cannot be instantiated as a user service"),
        ServiceType::Centrifugo => Box::new(centrifugo::CentrifugoService),
        ServiceType::Gitea => Box::new(gitea::GiteaService),
    }
}
