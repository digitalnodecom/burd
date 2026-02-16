//! MCP tool definitions for Burd

use serde_json::json;

use super::protocol::Tool;

/// Get all available MCP tools
pub fn get_tools() -> Vec<Tool> {
    vec![
        // ====================================================================
        // Usage Guide (IMPORTANT: Keep this first so AI agents see it)
        // ====================================================================
        Tool {
            name: "get_usage_guide".to_string(),
            description: "IMPORTANT: Call this first! Get instructions on how to use Burd for local development. Explains when to use Burd instead of manual server commands like 'php artisan serve'.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },

        // ====================================================================
        // Instance Tools
        // ====================================================================
        Tool {
            name: "list_instances".to_string(),
            description: "List all Burd service instances with their status, health, and configuration".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "create_instance".to_string(),
            description: "Create a new service instance (e.g., Redis, MariaDB, PostgreSQL, FrankenPHP, Meilisearch)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Human-readable name for the instance"
                    },
                    "port": {
                        "type": "integer",
                        "description": "Port number (must be >= 1024)"
                    },
                    "service_type": {
                        "type": "string",
                        "description": "Service type: redis, mariadb, postgresql, frankenphp, meilisearch, typesense, mongodb, memcached, valkey, minio, mailpit, beanstalkd, nodered, centrifugo"
                    },
                    "version": {
                        "type": "string",
                        "description": "Version to use (must be installed). Use get_service_versions to see available versions."
                    }
                },
                "required": ["name", "port", "service_type", "version"]
            }),
        },
        Tool {
            name: "start_instance".to_string(),
            description: "Start a service instance by ID".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Instance UUID (from list_instances)"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "stop_instance".to_string(),
            description: "Stop a running service instance".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Instance UUID"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "restart_instance".to_string(),
            description: "Restart a service instance (stop and start)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Instance UUID"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "delete_instance".to_string(),
            description: "Delete a service instance (stops it first if running)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Instance UUID"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "get_instance_logs".to_string(),
            description: "Get recent logs from a service instance".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Instance UUID"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "get_instance_env".to_string(),
            description: "Get environment variables and connection strings for an instance (DATABASE_URL, REDIS_URL, etc.)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Instance UUID"
                    }
                },
                "required": ["id"]
            }),
        },

        // ====================================================================
        // Domain Tools
        // ====================================================================
        Tool {
            name: "list_domains".to_string(),
            description: "List all configured domains with their routing targets".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "create_domain".to_string(),
            description: "Create a new domain routing. Maps a subdomain (e.g., 'api' for api.burd) to an instance, port, or static files.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "subdomain": {
                        "type": "string",
                        "description": "Subdomain name (e.g., 'api' creates api.burd)"
                    },
                    "target_type": {
                        "type": "string",
                        "enum": ["instance", "port", "static"],
                        "description": "Type of target: 'instance' (route to service), 'port' (proxy to port), 'static' (serve files)"
                    },
                    "target_value": {
                        "type": "string",
                        "description": "Target value: instance UUID, port number, or file path (depending on target_type)"
                    },
                    "ssl_enabled": {
                        "type": "boolean",
                        "description": "Enable HTTPS with auto-generated certificate (default: false)"
                    }
                },
                "required": ["subdomain", "target_type", "target_value"]
            }),
        },
        Tool {
            name: "update_domain".to_string(),
            description: "Update a domain's routing configuration".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Domain UUID"
                    },
                    "subdomain": {
                        "type": "string",
                        "description": "New subdomain name (optional)"
                    },
                    "target_type": {
                        "type": "string",
                        "description": "New target type (optional)"
                    },
                    "target_value": {
                        "type": "string",
                        "description": "New target value (optional)"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "delete_domain".to_string(),
            description: "Delete a domain routing".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Domain UUID"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "toggle_domain_ssl".to_string(),
            description: "Enable or disable SSL/HTTPS for a domain".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Domain UUID"
                    },
                    "ssl_enabled": {
                        "type": "boolean",
                        "description": "Whether to enable SSL"
                    }
                },
                "required": ["id", "ssl_enabled"]
            }),
        },

        // ====================================================================
        // Database Tools
        // ====================================================================
        Tool {
            name: "list_databases".to_string(),
            description: "List all databases inside running database instances (MariaDB, PostgreSQL). Use list_instances first to see available database servers.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "create_database".to_string(),
            description: "Create a new database inside a running database instance. Requires a database instance (mariadb, postgresql) to be running - use list_instances to check.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Database name (alphanumeric and underscores only)"
                    },
                    "instance_id": {
                        "type": "string",
                        "description": "Optional: specific database instance UUID. If not provided, uses first available."
                    }
                },
                "required": ["name"]
            }),
        },
        Tool {
            name: "drop_database".to_string(),
            description: "Drop/delete a database from a running database instance.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Database name to drop"
                    }
                },
                "required": ["name"]
            }),
        },
        Tool {
            name: "import_database".to_string(),
            description: "Import a SQL file into a database. The database must exist in a running MariaDB or PostgreSQL instance.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": {
                        "type": "string",
                        "description": "Database name to import into"
                    },
                    "sql_file": {
                        "type": "string",
                        "description": "Absolute path to the SQL file to import"
                    }
                },
                "required": ["database", "sql_file"]
            }),
        },
        Tool {
            name: "export_database".to_string(),
            description: "Export a database to a SQL file. Creates a dump of all tables and data from a MariaDB or PostgreSQL database.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": {
                        "type": "string",
                        "description": "Database name to export"
                    },
                    "output_file": {
                        "type": "string",
                        "description": "Path where the SQL dump will be saved (optional, defaults to {database}.sql in current directory)"
                    }
                },
                "required": ["database"]
            }),
        },

        // ====================================================================
        // Service Tools
        // ====================================================================
        Tool {
            name: "list_services".to_string(),
            description: "List all available service types that can be installed in Burd".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_service_versions".to_string(),
            description: "Get installed versions for a specific service type".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "service_type": {
                        "type": "string",
                        "description": "Service type (e.g., redis, mariadb, postgresql)"
                    }
                },
                "required": ["service_type"]
            }),
        },

        // ====================================================================
        // Status Tool
        // ====================================================================
        Tool {
            name: "get_status".to_string(),
            description: "Get overall Burd system status including DNS, proxy, and instance counts".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },

        // ====================================================================
        // Database Tool Execution
        // ====================================================================
        Tool {
            name: "execute_db_tool".to_string(),
            description: "Execute a database CLI tool (mysql, mysqldump, psql, pg_dump, etc.) with auto-connection to a running Burd database instance. Useful for running database commands, backups, and administrative tasks.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "service": {
                        "type": "string",
                        "enum": ["mysql", "mariadb", "postgres"],
                        "description": "Database service type. Use 'mysql' or 'mariadb' for MySQL/MariaDB tools, 'postgres' for PostgreSQL tools."
                    },
                    "tool": {
                        "type": "string",
                        "description": "Tool name to execute (e.g., mysql, mysqldump, mysqlimport, psql, pg_dump, createdb)"
                    },
                    "args": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Arguments to pass to the tool (e.g., database name, flags)"
                    }
                },
                "required": ["service", "tool"]
            }),
        },
        Tool {
            name: "list_db_tools".to_string(),
            description: "List available database CLI tools for a specific service (mysql/mariadb or postgres)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "service": {
                        "type": "string",
                        "enum": ["mysql", "mariadb", "postgres"],
                        "description": "Database service type"
                    }
                },
                "required": ["service"]
            }),
        },
    ]
}
