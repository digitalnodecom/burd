# Changelog

All notable changes to Burd will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [1.8.0] - 2026-07-16

First release since 1.2.5; rolls up the 1.3–1.7 work plus a security
hardening pass across the privileged helper and the localhost control API.

### Security

- **Privileged helper now authenticates its caller.** The root helper
  verifies the connecting process (via `getpeereid`) and serves only the
  console user or root, closing an unauthenticated local privilege
  escalation through its world-connectable socket.
- **Localhost API rejects cross-site and DNS-rebinding requests.** New
  guard middleware turns away non-loopback `Host` and foreign `Origin`
  headers, closing CSRF/rebinding access to the unauthenticated API.
- **Proxy daemon runs Caddy from a root-owned path.** The binary launchd
  executes as root was moved out of the user-writable app directory so an
  unprivileged process can no longer swap it; the helper installs it
  root-owned and can verify its SHA-256.
- **Path-traversal hardening.** Validate the `version` parameter on binary
  download/delete, the resolver TLD and Caddy-permissions path in the
  helper, and database-tool names before they are executed.
- Wire optional per-version SHA-256 verification into the binary download
  path (`scripts/generate-checksums.sh` generates the digests).

### Added

- Expose the Mailpit inbox over the HTTP API and MCP tools.
- macOS menu-bar tray with instance overview and quick controls.
- On-demand macOS ARM (aarch64) build workflow.
- `SECURITY.md` disclosure policy.

### Fixed

- Sync `package-lock.json` so `npm ci` (and CI) works again.
- Correct the documented local TLD (`.burd`) and other README drift.

---

## [1.2.5] - 2026-02-17

### Fixed
- CI pipeline: apply cargo fmt, resolve all clippy warnings, fix doc tests
- Svelte type errors: missing toast references, async onMount return type,
  drag-and-drop handlers, missing type properties

### Changed
- CLI update system now uses GitHub Releases directly (removed Scaleway dependency)
- Updated all repository references to digitalnodecom/burd

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

[Unreleased]: https://github.com/digitalnodecom/burd/compare/v1.2.5...HEAD
[1.2.5]: https://github.com/digitalnodecom/burd/compare/v0.2.0...v1.2.5
[0.2.0]: https://github.com/digitalnodecom/burd/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/digitalnodecom/burd/releases/tag/v0.1.0
