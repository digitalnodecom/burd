# Burd vs Herd Pro vs Laravel Valet - Feature Comparison

A comprehensive comparison of local PHP development environments for macOS.

## Core Features

| Feature | Burd | Herd Pro | Valet |
|---------|:----:|:--------:|:-----:|
| **PHP Version Management** | Yes | Yes (7.4-8.4) | Yes |
| **Per-site PHP Pinning** | No | Yes | Yes |
| **Node Version Manager** | Yes (NVM) | Yes | No |
| **Custom TLD (.test/.burd)** | Yes | Yes | Yes |
| **Automatic HTTPS/SSL** | Yes (Caddy CA) | Yes | Yes |
| **Park Directories** | Yes | Yes | Yes |
| **Link Individual Sites** | Yes (via instances) | Yes | Yes |
| **Site Sharing/Tunnels** | Yes (frpc) | Yes (ngrok) | Yes (ngrok/Expose) |

## Services

| Service | Burd | Herd Pro | Valet |
|---------|:----:|:--------:|:-----:|
| **MySQL** | No | Yes | No |
| **MariaDB** | Yes | Yes | No |
| **PostgreSQL** | Yes | Yes | No |
| **MongoDB** | Yes | No | No |
| **Redis** | Yes | Yes | No |
| **Valkey** | Yes | No | No |
| **Memcached** | Yes | No | No |
| **Meilisearch** | Yes | Yes | No |
| **Typesense** | Yes | Yes | No |
| **MinIO (S3)** | Yes | Yes | No |
| **Beanstalkd** | Yes | No | No |
| **Laravel Reverb** | No | Yes | No |

## Developer Tools

| Feature | Burd | Herd Pro | Valet |
|---------|:----:|:--------:|:-----:|
| **Mail Catching** | Yes (Mailpit) | Yes | No |
| **Dump Debugger (dd())** | No | Yes | No |
| **Log Viewer** | No | Yes | No |
| **Xdebug Integration** | No | Yes | No |
| **PHP Tinker Console** | Yes | No | No |
| **PM2 Integration** | Yes | No | No |
| **Node-RED** | Yes | No | No |

## Advanced Features

| Feature | Burd | Herd Pro | Valet |
|---------|:----:|:--------:|:-----:|
| **GUI Application** | Yes (Tauri) | Yes (native) | No (CLI only) |
| **CLI Tool** | Yes | Yes | Yes |
| **Config Sharing** | No | Yes (herd.yml) | No |
| **Forge Integration** | No | Yes | No |
| **Custom Drivers** | No | Yes | Yes |
| **Multiple Instances per Service** | Yes | Yes | No |

## What Makes Burd Different

### Unique Features in Burd

- **MongoDB Support** - NoSQL database management built-in
- **Valkey Support** - Redis-compatible alternative
- **Memcached Support** - Memory caching service
- **Beanstalkd Support** - Job queue system
- **PHP Tinker Console** - Interactive PHP REPL for Laravel, WordPress, and generic PHP projects
- **PM2 Integration** - Node.js process management
- **Node-RED** - Visual programming for IoT and automation
- **frpc Tunnels** - Self-hosted tunnel support with custom FRP servers

### Service Comparison Summary

| Category | Burd | Herd Pro | Valet |
|----------|------|----------|-------|
| Database Services | 3 (MariaDB, PostgreSQL, MongoDB) | 3 (MySQL, MariaDB, PostgreSQL) | 0 |
| Cache Services | 3 (Redis, Valkey, Memcached) | 1 (Redis) | 0 |
| Search Services | 2 (Meilisearch, Typesense) | 2 (Meilisearch, Typesense) | 0 |
| Storage Services | 1 (MinIO) | 1 (MinIO) | 0 |
| Queue Services | 1 (Beanstalkd) | 0 | 0 |
| **Total Services** | **10** | **7** | **0** |

## Pricing Comparison

| Product | Price |
|---------|-------|
| Burd | Free / Open Source |
| Herd Pro | €99/year (individual), €299/year (team of 10) |
| Laravel Valet | Free / Open Source |

## Target Audience

### Choose Burd if you:
- Want a free, open-source solution with a GUI
- Need multiple database options (MariaDB, PostgreSQL, MongoDB)
- Work with Node.js and need PM2 integration
- Want to use your own tunnel servers (frpc)
- Need a PHP Tinker console for quick code execution
- Prefer more cache options (Redis, Valkey, Memcached)

### Choose Herd Pro if you:
- Want official Laravel team support
- Need the dump debugger and log viewer
- Require Xdebug auto-detection
- Want Laravel Forge integration for deployment
- Need team configuration sharing via herd.yml
- Prefer a native macOS experience

### Choose Laravel Valet if you:
- Want a minimal, CLI-only solution
- Don't need database/cache management
- Prefer maximum simplicity
- Need custom driver support for non-Laravel frameworks

## Requirements

| Product | Requirements |
|---------|-------------|
| Burd | macOS, Homebrew |
| Herd Pro | macOS 12+ or Windows 10+ |
| Laravel Valet | macOS, Homebrew, Composer |

---

*Last updated: January 2025*
