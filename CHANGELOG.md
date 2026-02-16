# Changelog

All notable changes to Burd will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `burd upgrade` command for CLI self-update

---

## [0.2.0] - 2024-XX-XX

### Added
- **Project Analyzer** - Detect Laravel, WordPress, Bedrock, Symfony projects
- **Database Manager** - CLI commands for database operations
  - `burd db list` - List all databases
  - `burd db create <name>` - Create database
  - `burd db drop <name>` - Drop database
  - `burd db import <name> <file>` - Import SQL file
  - `burd db export <name>` - Export database
  - `burd db shell [name]` - Interactive database shell
- **Environment Manager** - Check and fix .env files
  - `burd env check` - Compare .env with Burd services
  - `burd env fix` - Interactive .env fixer
  - `burd env show` - Show relevant .env values
- **Project Scaffolding** - Create new projects
  - `burd new laravel <name>` - Create Laravel project
  - `burd new wordpress <name>` - Create WordPress site
  - `burd new bedrock <name>` - Create Bedrock project
- **Setup Wizard** - Full interactive project setup
  - `burd setup` - Configure everything in one command
- **Health Check** - Diagnose issues
  - `burd doctor` - Check services and project configuration
- **Enhanced `burd link`** - Smart project detection and setup
  - Detects project type automatically
  - Offers to create database
  - Offers to fix .env configuration
  - Copies .env.example if .env doesn't exist

### Changed
- `burd analyze` now shows detailed service compatibility

### Fixed
- Database port detection for Bedrock projects (DB_NAME vs DB_DATABASE)
- Host parsing when port is embedded in DB_HOST

---

## [0.1.0] - 2024-XX-XX

### Added
- Initial release of Burd
- **FrankenPHP** support with document root configuration
- **FrankenPHP Park** for automatic subdirectory domains
- **MariaDB** database service
- **PostgreSQL** database service
- **Redis** cache service
- **Mailpit** local mail testing
- **Meilisearch** search engine
- **Frpc** tunnel support for sharing sites
- **Caddy Proxy** for HTTPS and routing
- CLI commands:
  - `burd init` - Initialize development server
  - `burd link` - Link directory to domain
  - `burd unlink` - Remove link
  - `burd links` - List linked sites
  - `burd park` - Park directory for auto-domains
  - `burd forget` - Unpark directory
  - `burd parked` - List parked directories
  - `burd refresh` - Refresh parked directories
  - `burd status` - Show park status
  - `burd share` - Share site via tunnel
  - `burd analyze` - Analyze project
- macOS menu bar app with system tray
- Instance management (start/stop/configure)
- Domain management with SSL support
- Automatic hosts file management
- Helper process for privileged operations

---

[Unreleased]: https://github.com/digitalnodecom/burd/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/digitalnodecom/burd/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/digitalnodecom/burd/releases/tag/v0.1.0
