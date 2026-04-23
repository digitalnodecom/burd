//! Burd CLI - Local development server management from the command line
//!
//! Full documentation: docs/CLI.md
//!
//! Usage:
//!   burd analyze   Analyze the current project (detect type, config, issues)
//!   burd init      Create a development server for the current directory
//!   burd link      Link the current directory to a custom domain
//!   burd unlink    Remove the link for the current directory
//!   burd links     List all linked sites
//!   burd park      Park the current directory (auto-create domains for subdirectories)
//!   burd forget    Unpark the current directory
//!   burd parked    List all parked directories
//!   burd refresh   Refresh parked directories (check for new/removed projects)
//!   burd status    Show park status for current directory
//!   burd share     Share a site via tunnel
//!   burd db        Database management (list, create, drop, import, export, shell)
//!   burd env       Environment management (check, fix, show)

use burd_lib::cli;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "burd")]
#[command(author = "Burd")]
#[command(version = "0.21.0")]
#[command(about = "Local development server management CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze the current project
    ///
    /// Detects project type (Laravel, WordPress, Bedrock), parses configuration,
    /// and checks against Burd services for potential improvements.
    Analyze,

    /// Initialize a new development server in the current directory
    ///
    /// Creates a FrankenPHP instance pointing to this directory (or to a
    /// detected public/web root for Laravel/Symfony/Bedrock), sets up a
    /// domain with SSL enabled, and starts the site.
    Init {
        /// Skip enabling SSL on the new domain
        #[arg(long)]
        no_ssl: bool,
        /// Don't auto-start the instance after creating it
        #[arg(long)]
        no_start: bool,
        /// Override the document root (path, absolute or relative to cwd)
        #[arg(long, value_name = "PATH")]
        public_dir: Option<std::path::PathBuf>,
    },

    /// Park the current directory
    ///
    /// All subdirectories will automatically become domains.
    /// Requires FrankenPHP Park instance to be created in the Burd app first.
    Park,

    /// Unpark (forget) the current directory
    ///
    /// Removes the current directory from the list of parked directories
    /// and deletes all associated domains.
    Forget,

    /// List all parked directories
    ///
    /// Shows all directories that are currently parked along with their projects.
    Parked,

    /// Refresh parked directories
    ///
    /// Scans parked directories for new or removed projects.
    /// Full sync requires the Burd app to be running.
    Refresh,

    /// Show park status for current directory
    ///
    /// Displays whether the current directory is parked or is a project
    /// inside a parked directory.
    Status,

    /// Link the current directory to a custom domain
    ///
    /// Creates a FrankenPHP instance and domain for the current directory.
    /// Similar to 'burd init' but allows specifying a custom subdomain.
    ///
    /// Examples:
    ///   burd link           # Use directory name as subdomain
    ///   burd link myapp     # Use 'myapp' as subdomain
    ///   burd link myapp.burd  # Same as above (TLD is stripped)
    Link {
        /// Domain name (e.g., 'myapp' or 'myapp.burd')
        name: Option<String>,
        /// Skip enabling SSL on the new domain
        #[arg(long)]
        no_ssl: bool,
        /// Don't auto-start the instance after linking
        #[arg(long)]
        no_start: bool,
    },

    /// Unlink the current directory
    ///
    /// Removes the domain and instance created by 'burd link' or 'burd init'.
    Unlink,

    /// List all linked sites
    ///
    /// Shows all directories linked via 'burd link' or 'burd init'.
    Links,

    /// Start an instance
    ///
    /// Starts the named instance, or the instance tied to the current
    /// directory when NAME is omitted.
    Start {
        /// Instance name or domain (optional)
        name: Option<String>,
    },

    /// Stop an instance
    Stop {
        /// Instance name or domain (optional)
        name: Option<String>,
    },

    /// Restart an instance
    Restart {
        /// Instance name or domain (optional)
        name: Option<String>,
    },

    /// Show recent logs for an instance
    ///
    /// Resolves NAME the same way as start/stop/restart (name, UUID, subdomain,
    /// or current directory when omitted). `--follow` polls the daemon once
    /// per second and streams new content until interrupted.
    Logs {
        /// Instance name or domain (optional)
        name: Option<String>,
        /// Number of trailing lines to show (default: 100)
        #[arg(long, default_value_t = 100)]
        lines: usize,
        /// Tail the log, printing new entries as they arrive
        #[arg(short, long)]
        follow: bool,
    },

    /// Update instance settings
    ///
    /// Currently supports `--php-version`, `--port`, and `--name`. Mirrors the
    /// MCP `update_instance` tool.
    Update {
        /// Instance name or domain (optional; defaults to cwd's instance)
        name: Option<String>,
        /// New PHP (FrankenPHP) version — must be already installed
        #[arg(long)]
        php_version: Option<String>,
        /// New port
        #[arg(long)]
        port: Option<u16>,
        /// Rename the instance
        #[arg(long = "name", value_name = "NEW_NAME")]
        new_name: Option<String>,
    },

    /// List installed versions of Burd services
    ///
    /// Examples:
    ///   burd versions                   # All services + installed versions
    ///   burd versions --service frankenphp
    Versions {
        /// Restrict output to a single service type (e.g. frankenphp, mariadb)
        #[arg(long)]
        service: Option<String>,
    },

    /// Enable HTTPS for a domain
    ///
    /// Enables SSL/TLS for the specified domain or current directory's domain.
    ///
    /// Examples:
    ///   burd secure           # Enable SSL for current directory's domain
    ///   burd secure myapp     # Enable SSL for myapp.burd
    ///   burd secure myapp.burd  # Same as above
    Secure {
        /// Domain name (optional, defaults to current directory's domain)
        name: Option<String>,
    },

    /// Disable HTTPS for a domain
    ///
    /// Disables SSL/TLS for the specified domain or current directory's domain.
    ///
    /// Examples:
    ///   burd unsecure           # Disable SSL for current directory's domain
    ///   burd unsecure myapp     # Disable SSL for myapp.burd
    Unsecure {
        /// Domain name (optional, defaults to current directory's domain)
        name: Option<String>,
    },

    /// Open a site in the default browser
    ///
    /// Opens the specified domain or current directory's domain in browser.
    ///
    /// Examples:
    ///   burd open           # Open current directory's domain
    ///   burd open myapp     # Open myapp.burd
    ///   burd open myapp.burd  # Same as above
    Open {
        /// Domain name (optional, defaults to current directory's domain)
        name: Option<String>,
    },

    /// Proxy a domain to a local port
    ///
    /// Creates a domain that proxies to localhost on the specified port.
    /// Unlike 'link', this doesn't create a FrankenPHP instance.
    ///
    /// Examples:
    ///   burd proxy myapi 3000        # Proxy myapi.burd -> localhost:3000
    ///   burd proxy myapi.burd 8080   # Proxy myapi.burd -> localhost:8080
    Proxy {
        /// Domain name (e.g., 'myapi' or 'myapi.burd')
        name: String,
        /// Port to proxy to
        port: u16,
    },

    /// Remove a proxied domain
    ///
    /// Removes a domain created by 'burd proxy'.
    Unproxy {
        /// Domain name to remove
        name: String,
    },

    /// List all proxied domains
    ///
    /// Shows domains created via 'burd proxy' (port-based proxies).
    Proxies,

    /// Create a new project from template
    ///
    /// Scaffolds a new Laravel, WordPress, or Bedrock project.
    New {
        /// Project type (laravel, wordpress, bedrock)
        template: String,

        /// Project name (will be used as directory name)
        name: String,
    },

    /// Full interactive project setup wizard
    ///
    /// Analyzes the project and guides you through:
    /// - Creating FrankenPHP instance and domain
    /// - Setting up database
    /// - Configuring Redis for cache/sessions
    /// - Configuring Mailpit for local mail
    /// - Running migrations (Laravel)
    Setup,

    /// Health check for Burd services and current project
    ///
    /// Diagnoses issues with:
    /// - Service instances (running, ports)
    /// - Current project configuration
    /// - Database connectivity
    /// - Cache and mail setup
    Doctor,

    /// Update the burd CLI to the latest version
    ///
    /// Checks for updates and installs if available.
    Upgrade {
        /// Only check for updates, don't install
        #[arg(short, long)]
        check: bool,
    },

    /// Share a site via tunnel
    ///
    /// Exposes a local site to the internet via frpc tunnel.
    /// Requires: frpc instance installed, running, and connected.
    Share {
        /// Custom subdomain for the tunnel (optional, random if not specified)
        #[arg(short, long)]
        subdomain: Option<String>,
    },

    /// Database management commands
    ///
    /// Manage databases on Burd's MariaDB or PostgreSQL instances.
    #[command(subcommand)]
    Db(DbCommands),

    /// Environment file management
    ///
    /// Check and fix .env files against Burd services.
    #[command(subcommand)]
    Env(EnvCommands),

    /// Run MCP server for AI agent integration
    ///
    /// Starts an MCP (Model Context Protocol) server that communicates via stdio.
    /// This allows AI agents like Claude to control Burd programmatically.
    ///
    /// The Burd desktop application must be running for this to work.
    ///
    /// Configure in Claude Desktop's config:
    ///   {
    ///     "mcpServers": {
    ///       "burd": {
    ///         "command": "/usr/local/bin/burd",
    ///         "args": ["mcp"]
    ///       }
    ///     }
    ///   }
    Mcp,

    /// Run MySQL/MariaDB tools with auto-connection
    ///
    /// Executes any MySQL/MariaDB tool (mysql, mysqldump, mysqlimport, etc.)
    /// with automatic connection to a running Burd database instance.
    ///
    /// Examples:
    ///   burd mysql mysql                    # Open MySQL shell
    ///   burd mysql mysql mydb               # Open shell for specific database
    ///   burd mysql mysqldump mydb           # Dump a database
    ///   burd mysql mysqlimport mydb data.txt # Import data
    ///   burd mysql list                     # List available tools
    ///
    /// Connection parameters are automatically injected from the running instance.
    /// You can override them with explicit flags (--host, --port, etc.)
    ///
    /// Note: clap would otherwise intercept `-h`/`-V` before the underlying
    /// tool sees them. Help is disabled on this subcommand — use `--help`
    /// (long form) for burd's help, or pass `--` before tool args to be safe:
    ///   burd mysql mysql -- -h 127.0.0.1 -P 3306
    #[command(
        name = "mysql",
        alias = "mariadb",
        disable_help_flag = true,
        disable_version_flag = true
    )]
    Mysql {
        /// Tool name (mysql, mysqldump, mysqlimport, etc.) or 'list' to show available tools
        tool: String,
        /// Arguments to pass to the tool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Run PostgreSQL tools with auto-connection
    ///
    /// Executes any PostgreSQL tool (psql, pg_dump, pg_restore, etc.)
    /// with automatic connection to a running Burd database instance.
    ///
    /// Examples:
    ///   burd postgres psql                  # Open PostgreSQL shell
    ///   burd postgres psql mydb             # Open shell for specific database
    ///   burd postgres pg_dump mydb          # Dump a database
    ///   burd postgres createdb newdb        # Create a database
    ///   burd postgres list                  # List available tools
    ///
    /// Connection parameters are automatically injected from the running instance.
    /// You can override them with explicit flags (--host, --port, etc.)
    ///
    /// Note: clap would otherwise intercept `-h`/`-V` before psql sees them.
    /// Help is disabled on this subcommand — use `--help` for burd's help,
    /// or pass `--` before tool args:
    ///   burd postgres psql -- -h 127.0.0.1 -p 5432
    #[command(
        name = "postgres",
        alias = "pg",
        disable_help_flag = true,
        disable_version_flag = true
    )]
    Postgres {
        /// Tool name (psql, pg_dump, createdb, etc.) or 'list' to show available tools
        tool: String,
        /// Arguments to pass to the tool
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

/// Environment subcommands
#[derive(Subcommand)]
enum EnvCommands {
    /// Check .env against Burd services
    ///
    /// Compares your project's .env file with running Burd services
    /// and reports any mismatches.
    Check,

    /// Fix .env issues interactively
    ///
    /// Prompts for each issue found and offers to fix it.
    Fix,

    /// Show relevant .env values
    ///
    /// Displays database, cache, mail, and search settings.
    Show,
}

/// Engine selector for `burd db create`
#[derive(Copy, Clone, Debug, clap::ValueEnum)]
enum DbEngineArg {
    Mariadb,
    Postgres,
}

impl From<DbEngineArg> for burd_lib::db_manager::DbType {
    fn from(v: DbEngineArg) -> Self {
        match v {
            DbEngineArg::Mariadb => burd_lib::db_manager::DbType::MariaDB,
            DbEngineArg::Postgres => burd_lib::db_manager::DbType::PostgreSQL,
        }
    }
}

/// Database subcommands
#[derive(Subcommand)]
enum DbCommands {
    /// List all databases
    List,

    /// Create a new database
    Create {
        /// Database name
        name: String,

        /// Engine to use when multiple are configured (mariadb | postgres)
        #[arg(long, value_enum)]
        engine: Option<DbEngineArg>,

        /// Pin to a specific Burd instance by name (overrides --engine)
        #[arg(long, value_name = "NAME")]
        instance: Option<String>,
    },

    /// Drop a database
    Drop {
        /// Database name
        name: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,

        /// Restrict search to a specific engine
        #[arg(long, value_enum)]
        engine: Option<DbEngineArg>,

        /// Restrict search to a specific Burd instance
        #[arg(long, value_name = "NAME")]
        instance: Option<String>,
    },

    /// Import SQL file into database
    Import {
        /// Database name
        name: String,

        /// Path to SQL file
        file: String,

        /// Engine to use when the database doesn't exist yet
        #[arg(long, value_enum)]
        engine: Option<DbEngineArg>,

        /// Burd instance to use when the database doesn't exist yet
        #[arg(long, value_name = "NAME")]
        instance: Option<String>,
    },

    /// Export database to SQL file
    Export {
        /// Database name
        name: String,

        /// Output file (default: <name>.sql)
        #[arg(short, long)]
        output: Option<String>,

        /// Restrict search to a specific engine
        #[arg(long, value_enum)]
        engine: Option<DbEngineArg>,

        /// Restrict search to a specific Burd instance
        #[arg(long, value_name = "NAME")]
        instance: Option<String>,
    },

    /// Open interactive database shell
    Shell {
        /// Database name (optional)
        name: Option<String>,

        /// Engine to open the shell against
        #[arg(long, value_enum)]
        engine: Option<DbEngineArg>,

        /// Burd instance to open the shell against
        #[arg(long, value_name = "NAME")]
        instance: Option<String>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Analyze => cli::run_analyze(),
        Commands::Init {
            no_ssl,
            no_start,
            public_dir,
        } => cli::run_init_with(cli::InitOptions {
            no_ssl,
            no_start,
            public_dir,
        }),
        Commands::Park => cli::run_park(),
        Commands::Forget => cli::run_forget(),
        Commands::Parked => cli::run_parked(),
        Commands::Refresh => cli::run_refresh(),
        Commands::Status => cli::run_status(),
        Commands::Link {
            name,
            no_ssl,
            no_start,
        } => cli::run_link_with(name, cli::LinkOptions { no_ssl, no_start }),
        Commands::Start { name } => cli::run_start(name),
        Commands::Stop { name } => cli::run_stop(name),
        Commands::Restart { name } => cli::run_restart(name),
        Commands::Logs {
            name,
            lines,
            follow,
        } => cli::run_logs(name, cli::LogsOptions { lines, follow }),
        Commands::Update {
            name,
            php_version,
            port,
            new_name,
        } => cli::run_update(
            name,
            cli::UpdateOptions {
                php_version,
                port,
                new_name,
            },
        ),
        Commands::Versions { service } => cli::run_service_versions(service),
        Commands::Unlink => cli::run_unlink(),
        Commands::Links => cli::run_links(),
        Commands::Secure { name } => cli::run_secure(name),
        Commands::Unsecure { name } => cli::run_unsecure(name),
        Commands::Open { name } => cli::run_open(name),
        Commands::Proxy { name, port } => cli::run_proxy(name, port),
        Commands::Unproxy { name } => cli::run_unproxy(name),
        Commands::Proxies => cli::run_proxies(),
        Commands::New { template, name } => cli::run_new(&template, &name),
        Commands::Setup => cli::run_setup(),
        Commands::Doctor => cli::run_doctor(),
        Commands::Upgrade { check } => cli::run_upgrade(check),
        Commands::Share { subdomain } => cli::run_share(subdomain),
        Commands::Db(db_cmd) => match db_cmd {
            DbCommands::List => cli::run_db_list(),
            DbCommands::Create {
                name,
                engine,
                instance,
            } => cli::run_db_create(&name, engine.map(Into::into), instance.as_deref()),
            DbCommands::Drop {
                name,
                force,
                engine,
                instance,
            } => cli::run_db_drop(&name, force, engine.map(Into::into), instance.as_deref()),
            DbCommands::Import {
                name,
                file,
                engine,
                instance,
            } => cli::run_db_import(&name, &file, engine.map(Into::into), instance.as_deref()),
            DbCommands::Export {
                name,
                output,
                engine,
                instance,
            } => cli::run_db_export(
                &name,
                output.as_deref(),
                engine.map(Into::into),
                instance.as_deref(),
            ),
            DbCommands::Shell {
                name,
                engine,
                instance,
            } => cli::run_db_shell(name.as_deref(), engine.map(Into::into), instance.as_deref()),
        },
        Commands::Env(env_cmd) => match env_cmd {
            EnvCommands::Check => cli::run_env_check(),
            EnvCommands::Fix => cli::run_env_fix(),
            EnvCommands::Show => cli::run_env_show(),
        },
        Commands::Mcp => cli::run_mcp(),
        Commands::Mysql { tool, args } => {
            if tool == "list" {
                cli::list_mysql_tools()
            } else {
                cli::run_mysql(&tool, args)
            }
        }
        Commands::Postgres { tool, args } => {
            if tool == "list" {
                cli::list_postgres_tools()
            } else {
                cli::run_postgres(&tool, args)
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
