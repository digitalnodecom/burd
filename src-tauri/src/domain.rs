//! Domain name generation and management for custom TLDs
//!
//! Handles converting instance names to valid domain slugs and managing
//! domain assignments to avoid conflicts.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Default TLD for local development
pub const DEFAULT_TLD: &str = "burd";

/// Default DNS server port (unprivileged)
pub const DEFAULT_DNS_PORT: u16 = 5354;

/// Default proxy server port (unprivileged)
pub const DEFAULT_PROXY_PORT: u16 = 8080;

/// Domain information for an instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainInfo {
    /// Auto-generated domain from instance name
    pub generated: String,
    /// User-specified custom domain (without TLD)
    pub custom: Option<String>,
    /// The effective domain being used (with TLD)
    pub effective: String,
    /// Whether domain routing is enabled for this instance
    pub enabled: bool,
    /// Full URL to access the service
    pub url: String,
}

/// Manages domain name generation and conflict resolution
#[derive(Debug, Clone)]
pub struct DomainManager {
    /// Set of currently registered domains (without TLD)
    registered: HashSet<String>,
    /// The proxy port for URL generation
    proxy_port: u16,
}

impl DomainManager {
    pub fn new(proxy_port: u16) -> Self {
        Self {
            registered: HashSet::new(),
            proxy_port,
        }
    }

    /// Generate a domain slug from an instance name
    ///
    /// Examples:
    /// - "My API" -> "my-api"
    /// - "Meilisearch Dev" -> "meilisearch-dev"
    /// - "Test_Server 123" -> "test-server-123"
    pub fn slugify(name: &str) -> String {
        slug::slugify(name)
    }

    /// Generate the full domain for an instance name
    ///
    /// Example: "My API" -> "my-api.burd"
    pub fn generate_domain(name: &str, tld: &str) -> String {
        format!("{}.{}", Self::slugify(name), tld)
    }

    /// Get a unique domain, appending a number if needed to avoid conflicts
    ///
    /// Example: If "my-api" is taken, returns "my-api-2"
    pub fn get_unique_slug(&self, name: &str) -> String {
        let base_slug = Self::slugify(name);

        if !self.registered.contains(&base_slug) {
            return base_slug;
        }

        // Find next available number
        let mut counter = 2;
        loop {
            let candidate = format!("{}-{}", base_slug, counter);
            if !self.registered.contains(&candidate) {
                return candidate;
            }
            counter += 1;

            // Safety limit
            if counter > 1000 {
                // Use a more unique suffix
                return format!("{}-{}", base_slug, uuid::Uuid::new_v4().to_string().split('-').next().unwrap());
            }
        }
    }

    /// Register a domain as in-use
    pub fn register(&mut self, slug: &str) {
        self.registered.insert(slug.to_string());
    }

    /// Unregister a domain
    pub fn unregister(&mut self, slug: &str) {
        self.registered.remove(slug);
    }

    /// Check if a domain slug is available
    pub fn is_available(&self, slug: &str) -> bool {
        !self.registered.contains(slug)
    }

    /// Validate a domain slug
    ///
    /// Returns an error message if invalid, None if valid
    pub fn validate_slug(slug: &str) -> Option<String> {
        if slug.is_empty() {
            return Some("Domain cannot be empty".to_string());
        }

        if slug.len() > 63 {
            return Some("Domain must be 63 characters or less".to_string());
        }

        // Must start with alphanumeric
        if !slug.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false) {
            return Some("Domain must start with a letter or number".to_string());
        }

        // Must end with alphanumeric
        if !slug.chars().last().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false) {
            return Some("Domain must end with a letter or number".to_string());
        }

        // Only lowercase alphanumeric, hyphens, and periods
        for c in slug.chars() {
            if !c.is_ascii_lowercase() && !c.is_ascii_digit() && c != '-' && c != '.' {
                return Some("Domain can only contain lowercase letters, numbers, hyphens, and periods".to_string());
            }
        }

        // No consecutive hyphens
        if slug.contains("--") {
            return Some("Domain cannot contain consecutive hyphens".to_string());
        }

        None
    }

    /// Get domain info for an instance
    pub fn get_domain_info(
        &self,
        instance_name: &str,
        custom_domain: Option<&str>,
        enabled: bool,
        tld: &str,
    ) -> DomainInfo {
        let generated = Self::slugify(instance_name);
        let effective_slug = custom_domain.unwrap_or(&generated);
        let effective = format!("{}.{}", effective_slug, tld);
        let url = format!("http://{}:{}", effective, self.proxy_port);

        DomainInfo {
            generated: format!("{}.{}", generated, tld),
            custom: custom_domain.map(|s| s.to_string()),
            effective,
            enabled,
            url,
        }
    }

    /// Get all registered domains
    pub fn list_registered(&self) -> Vec<String> {
        self.registered.iter().cloned().collect()
    }
}

impl Default for DomainManager {
    fn default() -> Self {
        Self::new(DEFAULT_PROXY_PORT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(DomainManager::slugify("My API"), "my-api");
        assert_eq!(DomainManager::slugify("Meilisearch Dev"), "meilisearch-dev");
        assert_eq!(DomainManager::slugify("Test_Server 123"), "test-server-123");
        assert_eq!(DomainManager::slugify("UPPERCASE"), "uppercase");
    }

    #[test]
    fn test_generate_domain() {
        assert_eq!(DomainManager::generate_domain("My API", "burd"), "my-api.burd");
    }

    #[test]
    fn test_unique_slug() {
        let mut manager = DomainManager::default();

        assert_eq!(manager.get_unique_slug("My API"), "my-api");
        manager.register("my-api");

        assert_eq!(manager.get_unique_slug("My API"), "my-api-2");
        manager.register("my-api-2");

        assert_eq!(manager.get_unique_slug("My API"), "my-api-3");
    }

    #[test]
    fn test_validate_slug() {
        assert!(DomainManager::validate_slug("my-api").is_none());
        assert!(DomainManager::validate_slug("api123").is_none());

        assert!(DomainManager::validate_slug("").is_some());
        assert!(DomainManager::validate_slug("-api").is_some());
        assert!(DomainManager::validate_slug("api-").is_some());
        assert!(DomainManager::validate_slug("my--api").is_some());
        assert!(DomainManager::validate_slug("MY-API").is_some());
    }
}
