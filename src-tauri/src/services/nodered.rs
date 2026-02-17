use crate::config::{Instance, ServiceType};
use crate::services::{
    DownloadMethod, HealthCheck, ProcessManager, ServiceDefinition, VersionSource,
};
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct NodeRedService;

impl NodeRedService {
    /// Get the PM2 process name for a Node-RED instance
    pub fn get_pm2_name(instance: &Instance) -> String {
        // Use first 8 chars of UUID for uniqueness
        let short_id = instance.id.to_string()[..8].to_string();
        format!("burd-nodered-{}", short_id)
    }

    /// Get the path to the node-red script within an instance
    pub fn get_node_red_script(data_dir: &Path) -> std::path::PathBuf {
        // After copying source and running npm install, use red.js as entry point
        let red_js = data_dir.join("red.js");
        if red_js.exists() {
            return red_js;
        }
        // Fallback to npm bin if installed via npm
        data_dir.join("node_modules").join(".bin").join("node-red")
    }

    /// Generate settings.js for a Node-RED instance
    pub fn generate_settings(instance: &Instance, data_dir: &Path) -> Result<(), String> {
        let flow_file = instance
            .config
            .get("flow_file")
            .and_then(|v| v.as_str())
            .unwrap_or("flows.json");

        let settings_content = format!(
            r#"module.exports = {{
    uiPort: {},
    userDir: "{}",
    flowFile: "{}",
    httpAdminRoot: "/",
    httpNodeRoot: "/api",
    debugMaxLength: 1000,
    functionGlobalContext: {{}},
    exportGlobalContextKeys: false,
    logging: {{
        console: {{
            level: "info",
            metrics: false,
            audit: false
        }}
    }},
    editorTheme: {{
        projects: {{
            enabled: false
        }}
    }}
}};
"#,
            instance.port,
            data_dir.to_string_lossy().replace('\\', "\\\\"),
            flow_file
        );

        let settings_path = data_dir.join("settings.js");
        fs::write(&settings_path, settings_content)
            .map_err(|e| format!("Failed to write settings.js: {}", e))?;

        Ok(())
    }

    /// Initialize a Node-RED instance by copying source and running npm install
    pub fn init_instance(data_dir: &Path, version: &str) -> Result<(), String> {
        use crate::config::{get_versioned_binary_dir, ServiceType};

        // Get the downloaded Node-RED source directory
        let source_dir = get_versioned_binary_dir(ServiceType::NodeRed, version)?;

        if !source_dir.exists() {
            return Err(format!(
                "Node-RED {} not downloaded. Please download it first from the Services page.",
                version
            ));
        }

        // The zip extracts to node-red/ subfolder, check if that exists
        let node_red_src = source_dir.join("node-red");
        let actual_source = if node_red_src.exists() {
            node_red_src
        } else {
            source_dir.clone()
        };

        // Create data directory if it doesn't exist
        fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;

        // Copy Node-RED source to instance directory using shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
        let copy_cmd = format!(
            "cp -r \"{}\"/* \"{}\"/ 2>/dev/null; cp -r \"{}\"/@* \"{}\"/ 2>/dev/null; true",
            actual_source.to_string_lossy(),
            data_dir.to_string_lossy(),
            source_dir.to_string_lossy(), // Also copy @node-red/* packages
            data_dir.to_string_lossy()
        );

        let _copy_output = Command::new(&shell)
            .args(["-c", &copy_cmd])
            .output()
            .map_err(|e| format!("Failed to copy Node-RED source: {}", e))?;

        // Check if we have at least package.json
        if !data_dir.join("package.json").exists() {
            // Try direct copy as fallback
            Self::copy_dir_contents(&actual_source, data_dir)?;
        }

        // Get NVM init script if available
        let nvm_init = crate::pm2::get_nvm_init_script();

        // Build npm install command
        let home = std::env::var("HOME").unwrap_or_else(|_| "/Users/tj".to_string());
        let script = format!(
            r#"export HOME="{}" && {}npm install --production"#,
            home, nvm_init
        );

        // Run npm install
        let output = Command::new(&shell)
            .args(["-c", &script])
            .current_dir(data_dir)
            .env("HOME", &home)
            .output()
            .map_err(|e| format!("Failed to run npm install: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(format!("npm install failed:\n{}\n{}", stderr, stdout));
        }

        // Create .initialized marker
        let marker_path = data_dir.join(".initialized");
        fs::write(&marker_path, "")
            .map_err(|e| format!("Failed to create .initialized marker: {}", e))?;

        Ok(())
    }

    /// Helper to copy directory contents recursively
    fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
        if !src.is_dir() {
            return Err(format!("Source is not a directory: {}", src.display()));
        }

        fs::create_dir_all(dst)
            .map_err(|e| format!("Failed to create destination directory: {}", e))?;

        for entry in
            fs::read_dir(src).map_err(|e| format!("Failed to read source directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                Self::copy_dir_contents(&src_path, &dst_path)?;
            } else {
                fs::copy(&src_path, &dst_path)
                    .map_err(|e| format!("Failed to copy file {}: {}", src_path.display(), e))?;
            }
        }

        Ok(())
    }

    /// Check if a Node-RED instance is initialized
    pub fn is_initialized(data_dir: &Path) -> bool {
        data_dir.join(".initialized").exists()
    }
}

impl ServiceDefinition for NodeRedService {
    fn service_type(&self) -> ServiceType {
        ServiceType::NodeRed
    }

    fn display_name(&self) -> &'static str {
        "Node-RED"
    }

    fn default_port(&self) -> u16 {
        1880
    }

    fn binary_name(&self) -> &'static str {
        "node-red"
    }

    fn version_source(&self) -> VersionSource {
        // Use GitHub releases
        VersionSource::GitHubReleases("node-red/node-red")
    }

    fn download_method(&self, _version: &str, _arch: &str) -> DownloadMethod {
        // Node-RED is downloaded from GitHub as a zip file
        DownloadMethod::GitHubRelease {
            api_url: "https://api.github.com/repos/node-red/node-red/releases/tags/",
            asset_pattern: ".zip".to_string(),
            checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Http {
            path: "/".to_string(),
        }
    }

    fn start_args(&self, instance: &Instance, data_dir: &Path) -> Vec<String> {
        // Args passed to node-red command
        vec![
            "--userDir".to_string(),
            data_dir.to_string_lossy().to_string(),
            "--port".to_string(),
            instance.port.to_string(),
        ]
    }

    fn needs_init(&self) -> bool {
        true
    }

    fn init_command(&self, _data_dir: &Path) -> Option<(String, Vec<String>)> {
        // Initialization is handled by init_instance() which uses npm
        // Return None because we handle this specially
        None
    }

    fn process_manager(&self) -> ProcessManager {
        ProcessManager::Pm2
    }
}
