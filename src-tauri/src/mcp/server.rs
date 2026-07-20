//! MCP server implementation (stdio-based)

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use super::protocol::*;
use super::tools::get_tools;
use crate::api_client::BurdApiClient;

const PROTOCOL_VERSION: &str = "2024-11-05";

const USAGE_GUIDE: &str = r#"# Burd Local Development Guide

## What is Burd?
Burd is a local development environment manager for macOS. It handles running services (PHP, databases, Redis, etc.) and routing them to local domains like myapp.test.

## IMPORTANT: When to Use Burd
ALWAYS prefer Burd over running servers manually:

❌ DON'T: php artisan serve
✅ DO: Create a FrankenPHP instance + domain in Burd

❌ DON'T: npm run dev (for serving static sites)
✅ DO: Create a domain pointing to your dev server port, or serve static files directly

❌ DON'T: mysql -u root -p
✅ DO: Use Burd's MariaDB/PostgreSQL instance (get connection string with get_instance_env)

❌ DON'T: redis-server
✅ DO: Use Burd's Redis instance

## Typical Workflow

### For a PHP/Laravel project:
1. list_instances - Check if FrankenPHP instance exists
2. create_instance - Create FrankenPHP instance if needed (service_type: "frankenphp")
3. create_domain - Map subdomain to instance (e.g., subdomain: "myapp" → myapp.test)
4. create_instance - Create MariaDB if needed (service_type: "mariadb")
5. create_database - Create the project database
6. get_instance_env - Get DATABASE_URL to put in .env file

### For any web project needing a custom domain:
1. list_domains - See existing domain mappings
2. create_domain with target_type="port" - Route domain to your dev server's port

### For static sites:
1. create_domain with target_type="static" and target_value="/path/to/public"

## Domain Routing
- Burd uses a local TLD (default: .test)
- Subdomain "api" becomes api.test
- Can route to: instances, ports, or static files
- SSL/HTTPS available with auto-generated certificates

## Available Services
- frankenphp: PHP with Caddy (for Laravel, WordPress, etc.)
- mariadb: MySQL-compatible database
- postgresql: PostgreSQL database
- mongodb: MongoDB NoSQL database
- redis: Redis cache/queue
- meilisearch: Search engine
- minio: S3-compatible object storage
- mailpit: Email testing (catches all outgoing mail)
- memcached, valkey, typesense, beanstalkd, and more

## Working with Databases

Burd manages database servers as instances. You create databases inside these instances.

### Database Services
- mariadb: MySQL-compatible (use for Laravel, WordPress, most PHP apps)
- postgresql: PostgreSQL (for apps requiring PostgreSQL features)
- mongodb: MongoDB (NoSQL document database)

### Understanding the Hierarchy
1. **Database Instances** = Servers managed by Burd (created via create_instance)
2. **Databases** = Individual databases inside those instances (created via create_database)

### Database Workflow
1. list_instances - Check if a database instance exists
2. If not: create_instance with service_type="mariadb", "postgresql", or "mongodb"
3. list_databases - See existing databases across all SQL instances
4. create_database - Create a new database (MariaDB/PostgreSQL)
5. get_instance_env - Get DATABASE_URL connection string

### Import/Export (MariaDB/PostgreSQL)
- import_database: Import a SQL file into a database
- export_database: Export a database to a SQL file

### Connection Strings
Use get_instance_env to get connection strings for your .env file:
- DATABASE_URL: Full connection URL
- DB_HOST, DB_PORT, DB_USERNAME, DB_PASSWORD: Individual values

## Quick Reference
- list_instances: See all running services
- get_instance_env: Get connection strings (DATABASE_URL, REDIS_URL, etc.)
- list_domains: See all domain → target mappings
- list_databases: See all databases in running instances
- get_status: Check if Burd services are healthy
"#;

/// Run the MCP server loop
pub fn run_server() -> Result<(), String> {
    let client = BurdApiClient::new();

    // Check if Burd app is running
    if !client.is_available() {
        eprintln!("Error: Burd app is not running. Please start Burd first.");
        eprintln!("The MCP server requires the Burd desktop application to be running.");
        std::process::exit(1);
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("Error reading stdin: {}", e);
                continue;
            }
        };

        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let response = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                let _ = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        // JSON-RPC notifications (no `id`) MUST NOT receive a response.
        let is_notification = request.id.is_none();
        let response = handle_request(&client, request);
        if is_notification {
            continue;
        }
        if let Err(e) = writeln!(stdout, "{}", serde_json::to_string(&response).unwrap()) {
            eprintln!("Error writing response: {}", e);
        }
        let _ = stdout.flush();
    }

    Ok(())
}

fn handle_request(client: &BurdApiClient, request: JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => handle_initialize(request.id),
        "notifications/initialized" | "initialized" => {
            JsonRpcResponse::success(request.id, json!({}))
        }
        "tools/list" => handle_tools_list(request.id),
        "tools/call" => handle_tools_call(client, request.id, request.params),
        "ping" => JsonRpcResponse::success(request.id, json!({})),
        _ => JsonRpcResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

fn handle_initialize(id: Option<Value>) -> JsonRpcResponse {
    let result = InitializeResult {
        protocol_version: PROTOCOL_VERSION.to_string(),
        capabilities: ServerCapabilities {
            tools: ToolsCapability {
                list_changed: false,
            },
        },
        server_info: ServerInfo {
            name: "burd-mcp".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
    };

    JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
}

fn handle_tools_list(id: Option<Value>) -> JsonRpcResponse {
    let result = ListToolsResult { tools: get_tools() };
    JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
}

fn handle_tools_call(
    client: &BurdApiClient,
    id: Option<Value>,
    params: Option<Value>,
) -> JsonRpcResponse {
    let params: CallToolParams = match params {
        Some(p) => match serde_json::from_value(p) {
            Ok(p) => p,
            Err(e) => {
                return JsonRpcResponse::error(id, -32602, format!("Invalid params: {}", e));
            }
        },
        None => {
            return JsonRpcResponse::error(id, -32602, "Missing params");
        }
    };

    let result = execute_tool(client, &params.name, params.arguments);

    match result {
        Ok(content) => {
            let call_result = CallToolResult {
                content: vec![ToolContent::Text { text: content }],
                is_error: None,
            };
            JsonRpcResponse::success(id, serde_json::to_value(call_result).unwrap())
        }
        Err(e) => {
            let call_result = CallToolResult {
                content: vec![ToolContent::Text { text: e }],
                is_error: Some(true),
            };
            JsonRpcResponse::success(id, serde_json::to_value(call_result).unwrap())
        }
    }
}

/// Resolve an instance reference (UUID or name) to its UUID by hitting /instances.
/// Accepts either a UUID string or a case-insensitive instance name.
fn resolve_instance_id(client: &BurdApiClient, reference: &str) -> Result<String, String> {
    if uuid::Uuid::parse_str(reference).is_ok() {
        return Ok(reference.to_string());
    }
    let body = client.get("/instances")?;
    let list: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let arr = list.as_array().ok_or("Unexpected /instances response")?;
    let lower = reference.to_lowercase();
    let matches: Vec<&Value> = arr
        .iter()
        .filter(|i| {
            i.get("name")
                .and_then(|v| v.as_str())
                .map(|n| n.to_lowercase() == lower)
                .unwrap_or(false)
        })
        .collect();
    match matches.len() {
        0 => Err(format!("No instance named '{}'", reference)),
        1 => matches[0]
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| "Instance missing id".to_string()),
        _ => Err(format!(
            "Multiple instances named '{}' — pass the UUID instead",
            reference
        )),
    }
}

/// Resolve a domain reference (UUID, full domain, or subdomain) to its UUID.
fn resolve_domain_id(client: &BurdApiClient, reference: &str) -> Result<String, String> {
    if uuid::Uuid::parse_str(reference).is_ok() {
        return Ok(reference.to_string());
    }
    let body = client.get("/domains")?;
    let list: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let arr = list.as_array().ok_or("Unexpected /domains response")?;
    let lower = reference.to_lowercase();
    let matches: Vec<&Value> = arr
        .iter()
        .filter(|d| {
            let sub = d
                .get("subdomain")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase());
            let full = d
                .get("full_domain")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase());
            sub.as_deref() == Some(&lower) || full.as_deref() == Some(&lower)
        })
        .collect();
    match matches.len() {
        0 => Err(format!("No domain matching '{}'", reference)),
        1 => matches[0]
            .get("id")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| "Domain missing id".to_string()),
        _ => Err(format!(
            "Multiple domains match '{}' — pass the UUID",
            reference
        )),
    }
}

/// Pull `id` (or `name`) from args and resolve to a UUID.
fn arg_instance_id(client: &BurdApiClient, args: &Value) -> Result<String, String> {
    let raw = args
        .get("id")
        .or_else(|| args.get("name"))
        .and_then(|v| v.as_str())
        .ok_or("Missing 'id' or 'name' parameter")?;
    resolve_instance_id(client, raw)
}

fn arg_domain_id(client: &BurdApiClient, args: &Value) -> Result<String, String> {
    let raw = args
        .get("id")
        .or_else(|| args.get("subdomain"))
        .and_then(|v| v.as_str())
        .ok_or("Missing 'id' or 'subdomain' parameter")?;
    resolve_domain_id(client, raw)
}

fn arg_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("Missing '{}' parameter", key))
}

fn execute_tool(client: &BurdApiClient, name: &str, args: Option<Value>) -> Result<String, String> {
    let args = args.unwrap_or(json!({}));

    match name {
        // Instance tools
        "list_instances" => client.get("/instances"),
        "get_instance" => {
            let id = arg_instance_id(client, &args)?;
            client.get(&format!("/instances/{}", id))
        }
        "create_instance" => client.post("/instances", &args),
        "update_instance" => {
            let id = arg_instance_id(client, &args)?;
            // Build update body from provided fields (exclude id)
            let mut body = serde_json::Map::new();
            if let Some(v) = args.get("name") {
                body.insert("name".to_string(), v.clone());
            }
            if let Some(v) = args.get("port") {
                body.insert("port".to_string(), v.clone());
            }
            if let Some(v) = args.get("version") {
                body.insert("version".to_string(), v.clone());
            }
            if let Some(v) = args.get("domain") {
                body.insert("domain".to_string(), v.clone());
            }
            if let Some(v) = args.get("domain_enabled") {
                body.insert("domain_enabled".to_string(), v.clone());
            }
            if let Some(v) = args.get("config") {
                body.insert("config".to_string(), v.clone());
            }
            client.put(&format!("/instances/{}", id), &Value::Object(body))
        }
        "start_instance" => {
            let id = arg_instance_id(client, &args)?;
            client.post(&format!("/instances/{}/start", id), &json!({}))
        }
        "stop_instance" => {
            let id = arg_instance_id(client, &args)?;
            client.post(&format!("/instances/{}/stop", id), &json!({}))
        }
        "restart_instance" => {
            let id = arg_instance_id(client, &args)?;
            client.post(&format!("/instances/{}/restart", id), &json!({}))
        }
        "delete_instance" => {
            let id = arg_instance_id(client, &args)?;
            client.delete(&format!("/instances/{}", id))
        }
        "get_instance_logs" => {
            let id = arg_instance_id(client, &args)?;
            client.get(&format!("/instances/{}/logs", id))
        }
        "get_instance_env" => {
            let id = arg_instance_id(client, &args)?;
            client.get(&format!("/instances/{}/env", id))
        }
        "open_instance" => {
            let id = arg_instance_id(client, &args)?;
            client.post(&format!("/instances/{}/open", id), &json!({}))
        }
        "validate_instance" => {
            let id = arg_instance_id(client, &args)?;
            client.get(&format!("/instances/{}/validate", id))
        }
        "rename_instance" => {
            let id = arg_instance_id(client, &args)?;
            let new_name = arg_str(&args, "new_name")?;
            client.put(&format!("/instances/{}", id), &json!({ "name": new_name }))
        }
        "change_instance_version" => {
            let id = arg_instance_id(client, &args)?;
            let version = arg_str(&args, "version")?;
            client.put(
                &format!("/instances/{}", id),
                &json!({ "version": version }),
            )
        }

        // Domain tools
        "list_domains" => client.get("/domains"),
        "create_domain" => client.post("/domains", &args),
        "update_domain" => {
            let id = arg_domain_id(client, &args)?;
            client.put(&format!("/domains/{}", id), &args)
        }
        "delete_domain" => {
            let id = arg_domain_id(client, &args)?;
            client.delete(&format!("/domains/{}", id))
        }
        "toggle_domain_ssl" => {
            let id = arg_domain_id(client, &args)?;
            client.post(&format!("/domains/{}/ssl", id), &args)
        }

        // Service / binary tools
        "get_available_versions" => {
            let svc = arg_str(&args, "service_type")?;
            client.get(&format!("/services/{}/available", svc))
        }
        "download_binary" => {
            let svc = arg_str(&args, "service_type")?;
            let version = arg_str(&args, "version")?;
            client.post(
                &format!("/services/{}/versions/{}", svc, version),
                &json!({}),
            )
        }
        "delete_binary_version" => {
            let svc = arg_str(&args, "service_type")?;
            let version = arg_str(&args, "version")?;
            client.delete(&format!("/services/{}/versions/{}", svc, version))
        }

        // PHP CLI tools (PVM — PHP version manager)
        "get_php_cli_status" => client.get("/php/status"),
        "list_php_cli_versions" => client.get("/php/versions"),
        "list_available_php_cli_versions" => client.get("/php/versions/available"),
        "install_php_cli_version" => {
            let version = arg_str(&args, "version")?;
            client.post(&format!("/php/versions/{}", version), &json!({}))
        }
        "uninstall_php_cli_version" => {
            let version = arg_str(&args, "version")?;
            client.delete(&format!("/php/versions/{}", version))
        }
        "switch_php_cli_version" => {
            let version = arg_str(&args, "version")?;
            client.post(&format!("/php/default/{}", version), &json!({}))
        }
        "configure_php_cli_shell" => client.post("/php/shell", &json!({})),

        // Proxy tools
        "get_proxy_status" => client.get("/proxy/status"),
        "restart_proxy" => client.post("/proxy/restart", &json!({})),
        "get_port_conflicts" => client.get("/proxy/conflicts"),

        // Park tools
        "list_parked" => client.get("/parks"),
        "list_parked_projects" => client.get("/parks/projects"),

        // Tunnel tools
        "list_tunnels" => client.get("/tunnels"),
        "start_tunnels" => client.post("/tunnels/start", &json!({})),
        "stop_tunnels" => client.post("/tunnels/stop", &json!({})),
        "get_tunnel_status" => client.get("/tunnels/status"),

        // Stack tools
        "list_stacks" => client.get("/stacks"),
        "get_stack" => {
            let id = arg_str(&args, "id")?;
            client.get(&format!("/stacks/{}", id))
        }
        "create_stack" => client.post("/stacks", &args),
        "update_stack" => {
            let id = arg_str(&args, "id")?;
            client.put(&format!("/stacks/{}", id), &args)
        }
        "delete_stack" => {
            let id = arg_str(&args, "id")?;
            client.delete(&format!("/stacks/{}", id))
        }
        "start_stack" => {
            let id = arg_str(&args, "id")?;
            client.post(&format!("/stacks/{}/start", id), &json!({}))
        }
        "stop_stack" => {
            let id = arg_str(&args, "id")?;
            client.post(&format!("/stacks/{}/stop", id), &json!({}))
        }

        // Logs tools
        "list_log_sources" => client.get("/logs/sources"),
        "get_recent_logs" => {
            let mut params = Vec::new();
            if let Some(s) = args.get("source").and_then(|v| v.as_str()) {
                params.push(format!("source={}", urlencoding::encode(s)));
            }
            if let Some(l) = args.get("limit").and_then(|v| v.as_u64()) {
                params.push(format!("limit={}", l));
            }
            let path = if params.is_empty() {
                "/logs".to_string()
            } else {
                format!("/logs?{}", params.join("&"))
            };
            client.get(&path)
        }

        // Mail extras
        "delete_all_emails" => client.delete("/mail/messages"),

        // Database tools
        "list_databases" => client.get("/databases"),
        "create_database" => client.post("/databases", &args),
        "drop_database" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'name' parameter")?;
            client.delete(&format!("/databases/{}", name))
        }
        "list_database_extensions" => {
            let database = args
                .get("database")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'database' parameter")?;
            client.get(&format!(
                "/databases/{}/extensions",
                urlencoding::encode(database)
            ))
        }
        "enable_database_extension" => {
            let database = args
                .get("database")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'database' parameter")?;
            let extension = args
                .get("extension")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'extension' parameter")?;
            client.post(
                &format!(
                    "/databases/{}/extensions/{}",
                    urlencoding::encode(database),
                    urlencoding::encode(extension)
                ),
                &serde_json::json!({}),
            )
        }
        "disable_database_extension" => {
            let database = args
                .get("database")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'database' parameter")?;
            let extension = args
                .get("extension")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'extension' parameter")?;
            client.delete(&format!(
                "/databases/{}/extensions/{}",
                urlencoding::encode(database),
                urlencoding::encode(extension)
            ))
        }
        "import_database" => {
            let database = args
                .get("database")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'database' parameter")?;
            let sql_file = args
                .get("sql_file")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'sql_file' parameter")?;
            execute_cli_command(&["db", "import", database, sql_file])
        }
        "export_database" => {
            let database = args
                .get("database")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'database' parameter")?;
            match args.get("output_file").and_then(|v| v.as_str()) {
                Some(output) => execute_cli_command(&["db", "export", database, output]),
                None => execute_cli_command(&["db", "export", database]),
            }
        }

        // Service tools
        "list_services" => client.get("/services"),
        "get_service_versions" => {
            let service_type = args
                .get("service_type")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'service_type' parameter")?;
            client.get(&format!("/services/{}/versions", service_type))
        }

        // Mail (Mailpit)
        "get_mailpit_config" => client.get("/mail/config"),
        "list_emails" => {
            let mut params = Vec::new();
            if let Some(v) = args.get("start").and_then(|v| v.as_u64()) {
                params.push(format!("start={}", v));
            }
            if let Some(v) = args.get("limit").and_then(|v| v.as_u64()) {
                params.push(format!("limit={}", v));
            }
            if let Some(s) = args.get("search").and_then(|v| v.as_str()) {
                if !s.is_empty() {
                    params.push(format!("search={}", urlencoding::encode(s)));
                }
            }
            let path = if params.is_empty() {
                "/mail/messages".to_string()
            } else {
                format!("/mail/messages?{}", params.join("&"))
            };
            client.get(&path)
        }
        "get_email" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.get(&format!("/mail/messages/{}", id))
        }
        "delete_email" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.delete(&format!("/mail/messages/{}", id))
        }
        "mark_emails_read" => client.post("/mail/messages/read", &args),
        "get_unread_count" => client.get("/mail/unread-count"),

        // Status
        "get_status" => client.get("/status"),

        // Usage Guide (static response, no API call needed)
        "get_usage_guide" => Ok(USAGE_GUIDE.to_string()),

        // Database tool execution
        "execute_db_tool" => {
            let service = args
                .get("service")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'service' parameter")?;
            let tool = args
                .get("tool")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'tool' parameter")?;

            // Build the command based on service type
            let cmd = match service {
                "mysql" | "mariadb" => "mysql",
                "postgres" => "postgres",
                _ => return Err(format!("Unknown service: {}", service)),
            };

            // Build args list
            let mut cli_args = vec![cmd, tool];
            if let Some(tool_args) = args.get("args").and_then(|v| v.as_array()) {
                for arg in tool_args {
                    if let Some(s) = arg.as_str() {
                        cli_args.push(s);
                    }
                }
            }

            // Convert to owned strings
            let cli_args: Vec<&str> = cli_args.to_vec();
            execute_cli_command(&cli_args)
        }
        "list_db_tools" => {
            let service = args
                .get("service")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'service' parameter")?;

            let cmd = match service {
                "mysql" | "mariadb" => "mysql",
                "postgres" => "postgres",
                _ => return Err(format!("Unknown service: {}", service)),
            };

            execute_cli_command(&[cmd, "list"])
        }

        _ => Err(format!("Unknown tool: {}", name)),
    }
}

/// Execute a burd CLI command and return its output
fn execute_cli_command(args: &[&str]) -> Result<String, String> {
    use std::process::Command;

    let output = Command::new("/usr/local/bin/burd")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute burd CLI: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.is_empty() {
            Err(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(stderr.to_string())
        }
    }
}
