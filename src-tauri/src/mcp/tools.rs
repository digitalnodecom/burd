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
            description: "IMPORTANT: Call this first! Returns the guide on using Burd as a Docker alternative for local dev services (databases, caches, search, S3, mail) and PHP web apps. Explains when to reach for Burd instead of 'docker run' / docker-compose or manual commands like 'php artisan serve'.".to_string(),
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
            description: "List all Burd service instances with their status, health, port, and configuration. The equivalent of 'docker ps' — call this first to see what services are already running before creating new ones.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "create_instance".to_string(),
            description: "Stand up a new local service from a native binary — Burd's equivalent of 'docker run'. Use for databases (postgresql, mariadb, mysql, mongodb), caches/queues (redis, valkey, memcached, beanstalkd), search (meilisearch, typesense), S3 storage (minio), mail (mailpit), realtime (centrifugo), or a PHP web app (frankenphp). No container or image pull — it starts instantly. After creating, call start_instance to run it and get_instance_env for the connection string. Prefer this over 'docker run', docker-compose, or bare commands like 'redis-server'.".to_string(),
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
                        "description": "Service type: redis, mariadb, postgresql, frankenphp, meilisearch, typesense, mongodb, memcached, valkey, minio, mailpit, beanstalkd, centrifugo"
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
            name: "update_instance".to_string(),
            description: "Update a service instance's settings (name, port, version, domain, config). Only provide fields you want to change.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Instance UUID"
                    },
                    "name": {
                        "type": "string",
                        "description": "New instance name (optional)"
                    },
                    "port": {
                        "type": "integer",
                        "description": "New port number (optional)"
                    },
                    "version": {
                        "type": "string",
                        "description": "New version (optional, must be installed)"
                    },
                    "domain": {
                        "type": ["string", "null"],
                        "description": "Custom domain slug (optional, set to null to clear)"
                    },
                    "domain_enabled": {
                        "type": "boolean",
                        "description": "Enable/disable domain routing (optional)"
                    },
                    "config": {
                        "type": "object",
                        "description": "Service-specific config object (optional, replaces entire config)"
                    }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "start_instance".to_string(),
            description: "Start a stopped service instance so it accepts connections (≈ 'docker start'). Accepts the instance UUID or name.".to_string(),
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
            description: "Stop a running service instance (≈ 'docker stop'). Data is preserved; use start_instance to bring it back.".to_string(),
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
            description: "Restart a service instance (stop then start, ≈ 'docker restart'). Use after changing config/version to apply it.".to_string(),
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
            description: "Permanently remove a service instance (≈ 'docker rm'); stops it first if running. Use stop_instance instead if you only want to pause it.".to_string(),
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
            description: "Get recent stdout/stderr logs from a service instance (≈ 'docker logs'). Use to debug why a service failed to start or is misbehaving.".to_string(),
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
            description: "Get the ready-to-use connection string and environment variables for an instance (DATABASE_URL, REDIS_URL, host, port, username, password, etc.). Like reading a container's published ports and env vars in Docker. This is the payoff of create_instance — copy these straight into your app's .env file.".to_string(),
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
            description: "Route a local .test/.burd domain to a target — Burd's built-in reverse proxy (no nginx/Caddy config to write). Maps a subdomain (e.g. 'api' → api.test) to a Burd instance ('instance'), any local port to front an existing dev server ('port'), or a directory of files ('static'). Set ssl_enabled for automatic HTTPS. Prefer this over exposing raw localhost:PORT URLs.".to_string(),
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
            description: "List every database across all running database server instances (MariaDB, PostgreSQL). Each entry includes the database name, its on-disk size in bytes, and which instance it belongs to (instance_id, instance_name, service_type) — so you can inspect what exists and how much space it uses without a shell. Like 'SHOW DATABASES' / '\\l' across every server at once.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "list_database_users".to_string(),
            description: "List the users/roles across all running database server instances (MariaDB, PostgreSQL). Each entry gives the user name, host (MariaDB user@host; null for PostgreSQL), whether it's a superuser, whether it can log in, and the owning instance (instance_id, instance_name, service_type). Like 'SELECT User,Host FROM mysql.user' / '\\du' across every server at once — read-only introspection.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "create_database".to_string(),
            description: "Create a new database inside a running database server instance (≈ 'createdb' / 'CREATE DATABASE'). Requires a database instance (postgresql, mariadb, mysql) to already be running — use list_instances to check, create_instance to start one. After creating, use get_instance_env for the connection string.".to_string(),
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
            name: "list_database_extensions".to_string(),
            description: "List PostgreSQL extensions available in a database and whether each is enabled. Includes the bundled contrib extensions plus the ones Burd ships (pgvector for embeddings/vector search, pg_partman for partitioning).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": {
                        "type": "string",
                        "description": "Database name to inspect"
                    }
                },
                "required": ["database"]
            }),
        },
        Tool {
            name: "enable_database_extension".to_string(),
            description: "Enable a PostgreSQL extension on a database (runs CREATE EXTENSION). Use for pgvector ('vector'), pg_partman, or any bundled contrib extension (pgcrypto, uuid-ossp, hstore, postgis if installed, etc.). Idempotent.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": {
                        "type": "string",
                        "description": "Database name"
                    },
                    "extension": {
                        "type": "string",
                        "description": "Extension name, e.g. 'vector', 'pg_partman', 'pgcrypto', 'uuid-ossp'"
                    }
                },
                "required": ["database", "extension"]
            }),
        },
        Tool {
            name: "disable_database_extension".to_string(),
            description: "Disable a PostgreSQL extension on a database (runs DROP EXTENSION). Idempotent.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": {
                        "type": "string",
                        "description": "Database name"
                    },
                    "extension": {
                        "type": "string",
                        "description": "Extension name to disable"
                    }
                },
                "required": ["database", "extension"]
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
            description: "List every service type Burd can run (postgresql, mariadb, mysql, mongodb, redis, valkey, memcached, meilisearch, typesense, minio, mailpit, beanstalkd, centrifugo, frankenphp). Like browsing the available Docker images. Use to discover what you can create_instance.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "get_service_versions".to_string(),
            description: "Get the already-installed binary versions for a service type (like listing pulled image tags). create_instance requires one of these; if empty, use get_available_versions + download_binary to install one.".to_string(),
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
        // PHP CLI Version Manager (PVM) Tools
        // ====================================================================
        Tool {
            name: "get_php_cli_status".to_string(),
            description: "Get the status of Burd's command-line PHP version manager: how many CLI PHP versions are installed, which one is active (default), the PHP currently resolved in the terminal PATH, and whether shell integration is configured.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "list_php_cli_versions".to_string(),
            description: "List the command-line PHP versions installed via Burd's PHP version manager (like `nvm ls` for PHP). Marks which one is the active default.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "list_available_php_cli_versions".to_string(),
            description: "List command-line PHP versions available to install from Burd's binary releases (like `nvm ls-remote` for PHP). Returns the newest patch per minor version. Slow — hits GitHub.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "install_php_cli_version".to_string(),
            description: "Download and install a command-line PHP version via Burd's PHP version manager. These are Burd's custom static builds with a full extension set (redis, mongodb, imagick, intl, ffi, gd, pdo_mysql, pdo_pgsql, and more). Use list_available_php_cli_versions to see installable versions. Installing does not switch the active version — call switch_php_cli_version afterwards.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "version": {
                        "type": "string",
                        "description": "PHP version to install, e.g. '8.4.12' (see list_available_php_cli_versions)"
                    }
                },
                "required": ["version"]
            }),
        },
        Tool {
            name: "uninstall_php_cli_version".to_string(),
            description: "Uninstall a command-line PHP version previously installed via Burd's PHP version manager.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "version": {
                        "type": "string",
                        "description": "Installed PHP version to remove, e.g. '8.3.15'"
                    }
                },
                "required": ["version"]
            }),
        },
        Tool {
            name: "switch_php_cli_version".to_string(),
            description: "Set the active command-line PHP version (like `nvm use` for PHP). This is the global CLI PHP switch — it changes which `php` the terminal runs. Requires the version to be installed first (use install_php_cli_version).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "version": {
                        "type": "string",
                        "description": "Installed PHP version to activate, e.g. '8.4.12'"
                    }
                },
                "required": ["version"]
            }),
        },
        Tool {
            name: "configure_php_cli_shell".to_string(),
            description: "Configure shell integration so `php` in the terminal uses Burd's active version. Adds Burd's PHP directory to PATH in the user's shell profile (.zshrc/.bash_profile). Run once after installing your first CLI PHP version; open a new terminal for it to take effect.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },

        // ====================================================================
        // Mail Tools (Mailpit)
        // ====================================================================
        Tool {
            name: "get_mailpit_config".to_string(),
            description: "Get Mailpit SMTP/HTTP connection details for the running Mailpit instance. Use these in your app's .env (MAIL_HOST, MAIL_PORT).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        Tool {
            name: "list_emails".to_string(),
            description: "List captured emails from Mailpit. Supports pagination and full-text search. Use this to inspect outgoing mail during local development.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "start": { "type": "integer", "description": "Offset for pagination (default 0)" },
                    "limit": { "type": "integer", "description": "Max messages to return (default 50)" },
                    "search": { "type": "string", "description": "Optional Mailpit search query (subject/body/addr)" }
                },
                "required": []
            }),
        },
        Tool {
            name: "get_email".to_string(),
            description: "Get the full details (headers, text, HTML, attachments metadata) of a single captured email by ID.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Message ID from list_emails" }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "delete_email".to_string(),
            description: "Delete a single captured email by ID. (Bulk/delete-all is intentionally not exposed to MCP; use the HTTP API or the UI for that.)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Message ID to delete" }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "mark_emails_read".to_string(),
            description: "Mark one or more captured emails as read or unread.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ids": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Message IDs to update"
                    },
                    "read": { "type": "boolean", "description": "true = mark read, false = mark unread" }
                },
                "required": ["ids", "read"]
            }),
        },
        Tool {
            name: "get_unread_count".to_string(),
            description: "Count of unread captured emails in Mailpit.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },

        // ====================================================================
        // Status Tool
        // ====================================================================
        Tool {
            name: "get_status".to_string(),
            description: "Get overall Burd health in one call: DNS resolver, reverse-proxy daemon, and running/total instance counts. Good first check to confirm Burd is up before creating services.".to_string(),
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

        // ====================================================================
        // Instance lookup / lifecycle extras
        // (instance tools also accept `name` instead of UUID)
        // ====================================================================
        Tool {
            name: "get_instance".to_string(),
            description: "Get a single instance by UUID or name. Returns full details including config and runtime state.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Instance UUID" },
                    "name": { "type": "string", "description": "Instance name (alternative to id)" }
                }
            }),
        },
        Tool {
            name: "rename_instance".to_string(),
            description: "Rename an instance.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Instance UUID or name" },
                    "new_name": { "type": "string", "description": "New name" }
                },
                "required": ["new_name"]
            }),
        },
        Tool {
            name: "change_instance_version".to_string(),
            description: "Change the binary version an instance uses (must already be installed). Restart afterwards to apply.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Instance UUID or name" },
                    "version": { "type": "string", "description": "Installed version string (see get_service_versions)" }
                },
                "required": ["version"]
            }),
        },
        Tool {
            name: "validate_instance".to_string(),
            description: "Pre-flight check: verify the instance can be started (binary exists, is executable, working directory present). On failure returns an actionable hint naming the recovery tool to call (e.g. download_binary, update_instance). Use this when start_instance fails with an opaque error.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Instance UUID or name" }
                }
            }),
        },
        Tool {
            name: "open_instance".to_string(),
            description: "Open the instance's URL (its first routed domain, or http://127.0.0.1:port) in the user's default browser.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": "Instance UUID or name" }
                }
            }),
        },

        // ====================================================================
        // Service / binary management
        // ====================================================================
        Tool {
            name: "get_available_versions".to_string(),
            description: "List all downloadable versions for a service from upstream catalogs (GitHub releases, etc.). Slow — use sparingly.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "service_type": { "type": "string", "description": "e.g. 'redis', 'postgresql', 'frankenphp'" }
                },
                "required": ["service_type"]
            }),
        },
        Tool {
            name: "download_binary".to_string(),
            description: "Download and install a specific version of a service binary so create_instance can use it (≈ 'docker pull' for a tag). Long-running. Use get_available_versions to find versions first.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "service_type": { "type": "string" },
                    "version": { "type": "string" }
                },
                "required": ["service_type", "version"]
            }),
        },
        Tool {
            name: "delete_binary_version".to_string(),
            description: "Delete a downloaded binary version. Fails if any instance is using it.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "service_type": { "type": "string" },
                    "version": { "type": "string" }
                },
                "required": ["service_type", "version"]
            }),
        },

        // ====================================================================
        // Proxy / system
        // ====================================================================
        Tool {
            name: "get_proxy_status".to_string(),
            description: "Get reverse proxy daemon status (Caddy installed, daemon running, listening on 80/443).".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "restart_proxy".to_string(),
            description: "Restart the reverse proxy daemon. Use after changing instances or domains if routes look stale.".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "get_port_conflicts".to_string(),
            description: "List any non-Burd processes holding ports 80/443 (which would prevent the proxy from binding).".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },

        // ====================================================================
        // Park (parked directories — read-only via MCP)
        // ====================================================================
        Tool {
            name: "list_parked".to_string(),
            description: "List parked directories (FrankenPHP Park feature).".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "list_parked_projects".to_string(),
            description: "List discovered projects across all parked directories with their auto-generated domains.".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },

        // ====================================================================
        // Tunnels (frpc — share local services publicly)
        // ====================================================================
        Tool {
            name: "list_tunnels".to_string(),
            description: "List configured tunnels with their public URLs and running status.".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "start_tunnels".to_string(),
            description: "Start the frpc client to bring all tunnels up.".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "stop_tunnels".to_string(),
            description: "Stop the frpc client (takes all tunnels down).".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "get_tunnel_status".to_string(),
            description: "Get the frpc client status (running, error, etc.).".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },

        // ====================================================================
        // Stacks (groups of related instances)
        // ====================================================================
        Tool {
            name: "list_stacks".to_string(),
            description: "List all instance stacks with counts of total and running instances.".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "get_stack".to_string(),
            description: "Get a stack with the list of instances inside it.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "id": { "type": "string", "description": "Stack UUID" } },
                "required": ["id"]
            }),
        },
        Tool {
            name: "create_stack".to_string(),
            description: "Group existing instances into a named stack so they start/stop together — the equivalent of a docker-compose project. Build the instances with create_instance first, then bundle them here and use start_stack.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "instance_ids": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["name"]
            }),
        },
        Tool {
            name: "update_stack".to_string(),
            description: "Update a stack's name and/or description.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": ["string", "null"] }
                },
                "required": ["id"]
            }),
        },
        Tool {
            name: "delete_stack".to_string(),
            description: "Delete a stack. Member instances are kept (use delete_instance to remove them).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        },
        Tool {
            name: "start_stack".to_string(),
            description: "Start every instance in a stack in one call (≈ 'docker-compose up'). Returns per-instance results.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        },
        Tool {
            name: "stop_stack".to_string(),
            description: "Stop every instance in a stack in one call (≈ 'docker-compose down', but data is preserved). Returns per-instance results.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": { "id": { "type": "string" } },
                "required": ["id"]
            }),
        },

        // ====================================================================
        // Logs (system-wide, beyond per-instance)
        // ====================================================================
        Tool {
            name: "list_log_sources".to_string(),
            description: "List available log sources (caddy + per-service-type derived from running instances).".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        Tool {
            name: "get_recent_logs".to_string(),
            description: "Get recent log entries across sources. Optional 'source' filter ('caddy' or a service-type slug). Limit defaults to 200.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 5000 }
                }
            }),
        },

        // ====================================================================
        // Mail extras
        // ====================================================================
        Tool {
            name: "delete_all_emails".to_string(),
            description: "Delete every message in the Mailpit inbox.".to_string(),
            input_schema: json!({ "type": "object", "properties": {} }),
        },
    ]
}
