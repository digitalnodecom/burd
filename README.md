# Burd

A local development environment manager for macOS. Run PHP sites, databases, caches, and more with a single app.

Burd provides both a desktop GUI and a powerful CLI to manage FrankenPHP instances, databases, domains with automatic SSL, mail testing, tunnels, and other services — everything you need for local PHP development.

## Features

- **FrankenPHP** instances with per-project PHP configuration
- **Automatic domains** — link any directory to `yoursite.test` with one command
- **SSL out of the box** — powered by Caddy and mkcert
- **Park directories** — every subdirectory becomes a `.test` domain automatically
- **Database management** — create, drop, import, export, and shell into databases
- **Project scaffolding** — `burd new laravel myapp` and you're running
- **Environment fixer** — detects `.env` mismatches and fixes them
- **Tunnel sharing** — expose local sites to the internet via frpc
- **Health checks** — `burd doctor` diagnoses common issues
- **Self-updating CLI** — `burd upgrade` pulls the latest release from GitHub

## Supported Services

| Service | Default Port | Description |
|---------|-------------|-------------|
| FrankenPHP | 8000 | PHP application server |
| MariaDB | 3330 | SQL database |
| MySQL | 3306 | SQL database |
| PostgreSQL | 5432 | SQL database |
| MongoDB | 27017 | NoSQL database |
| Redis | 6379 | Cache and session store |
| Valkey | 6380 | Redis-compatible alternative |
| Memcached | 11211 | Memory cache |
| Mailpit | 8025 | Local mail testing (SMTP on 1025) |
| Meilisearch | 7700 | Full-text search engine |
| Typesense | 8108 | Full-text search engine |
| MinIO | 9000 | S3-compatible object storage |
| Beanstalkd | 11300 | Job queue |
| Centrifugo | 8000 | Real-time messaging |
| Node-RED | 1880 | Workflow automation |

## Installation

Download the latest `.dmg` from [Releases](https://github.com/digitalnodecom/burd/releases) and drag Burd to your Applications folder.

The CLI binary is also available as a standalone download for use without the desktop app.

## CLI

```bash
# Link a site
cd ~/projects/myapp
burd link myapp          # available at myapp.test

# Create a new project
burd new laravel blog    # scaffolds + links + configures

# Park a directory (every subfolder becomes a .test domain)
burd park

# Database operations
burd db create myapp
burd db import myapp dump.sql
burd db shell myapp

# Environment management
burd env check           # compare .env with running services
burd env fix             # interactive fixer

# SSL
burd secure myapp        # enable HTTPS
burd unsecure myapp      # disable HTTPS

# Proxy non-PHP apps
burd proxy api 3000      # proxy api.test to localhost:3000

# Share with the internet
burd share --subdomain myapp

# Diagnostics
burd doctor              # check services, config, connectivity
burd upgrade             # self-update to latest version
```

Run `burd --help` for the full command reference.

## Development

### Prerequisites

- Node.js 20+
- Rust (stable)
- Xcode Command Line Tools

### Setup

```bash
git clone https://github.com/digitalnodecom/burd.git
cd burd
npm install
npm run dev
```

### Building

```bash
# Desktop app
npm run tauri build

# CLI only
cd src-tauri && cargo build --release --bin burd
```

### Testing

```bash
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
npm run check
```

## Project Structure

```
burd/
├── src/                    # SvelteKit frontend
│   ├── lib/sections/       # UI sections (instances, domains, services, ...)
│   └── routes/             # Page routes
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── cli/            # CLI command implementations
│   │   ├── commands/       # Tauri IPC handlers
│   │   ├── services/       # Service integrations
│   │   └── bin/burd.rs     # CLI binary entry point
│   ├── helper/             # Privileged helper daemon
│   └── services.json       # Service definitions and versions
├── CHANGELOG.md
├── CONTRIBUTING.md
└── LICENSE
```

## License

MIT
