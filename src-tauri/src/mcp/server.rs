//! MCP server implementation (stdio-based)

use serde_json::{json, Value};
use std::io::{self, BufRead, Write};

use super::client::BurdApiClient;
use super::protocol::*;
use super::tools::get_tools;

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

        let response = handle_request(&client, request);
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
        "initialized" => JsonRpcResponse::success(request.id, json!({})),
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
            tools: ToolsCapability { list_changed: false },
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

fn execute_tool(client: &BurdApiClient, name: &str, args: Option<Value>) -> Result<String, String> {
    let args = args.unwrap_or(json!({}));

    match name {
        // Instance tools
        "list_instances" => client.get("/instances"),
        "create_instance" => client.post("/instances", &args),
        "start_instance" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.post(&format!("/instances/{}/start", id), &json!({}))
        }
        "stop_instance" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.post(&format!("/instances/{}/stop", id), &json!({}))
        }
        "restart_instance" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.post(&format!("/instances/{}/restart", id), &json!({}))
        }
        "delete_instance" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.delete(&format!("/instances/{}", id))
        }
        "get_instance_logs" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.get(&format!("/instances/{}/logs", id))
        }
        "get_instance_env" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.get(&format!("/instances/{}/env", id))
        }

        // Domain tools
        "list_domains" => client.get("/domains"),
        "create_domain" => client.post("/domains", &args),
        "update_domain" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.put(&format!("/domains/{}", id), &args)
        }
        "delete_domain" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.delete(&format!("/domains/{}", id))
        }
        "toggle_domain_ssl" => {
            let id = args
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or("Missing 'id' parameter")?;
            client.post(&format!("/domains/{}/ssl", id), &args)
        }

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
            let cli_args: Vec<&str> = cli_args.iter().map(|s| *s).collect();
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
