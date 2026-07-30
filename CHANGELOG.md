# Changelog

All notable changes to Burd will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [1.10.2] - 2026-07-30

### Fixed

- **`download_binary` accepts every version scheme the catalog emits.** The
  1.10.1 fix covered FrankenPHP's compound `8.5-1.12.4` but still rejected
  Caddy's v-prefixed upstream tags (`v2.11.4`) — the version validator was a
  brittle format allow-list. It is now a permissive-but-safe charset check
  (accepts `1.2.3`, `8.0`, `v2.11.4`, `8.5-1.12.4`, `system`; still blocks
  empty, path traversal, and separators), so any string from
  `get_available_versions` installs. Added a property test asserting every
  catalog version passes validation, which would have caught both cases.
- **Caddy binary download works.** The GitHub asset matcher treated the
  `caddy_*_mac_arm64.tar.gz` pattern literally, so the `*` never matched the
  real versioned asset name ("No binary found"). It now expands `*` wildcards.
  Combined with the validator fix, the HTTPS proxy's Caddy binary installs from
  the correct `v2.11.4` release tag.

### Note

The 1.10.1 fixes for the FrankenPHP version format and the "PHP 8.4"
mislabelling only take effect once the Burd app is fully restarted onto 1.10.1+
— a downloaded-but-not-yet-installed update keeps the old daemon (and its old
validator) serving the API/MCP.

---

## [1.10.1] - 2026-07-29

### Fixed

- **FrankenPHP no longer crashes on macOS 26.** The custom FrankenPHP 1.12.4
  builds (PHP 8.3/8.4/8.5) crashed nondeterministically (~75% of runs) with
  random SIGSEGV/SIGBUS on macOS 26 arm64, making PHP unservable. Cause: the
  builds enabled the mimalloc allocator, which is incompatible with
  FrankenPHP's Go runtime — the build flag intended to disable it (`MIMALLOC=0`)
  was misread as "enabled" because FrankenPHP treats any non-empty value as on.
  All six binaries (8.3/8.4/8.5 × arm64/x86_64) have been rebuilt without
  mimalloc and verified stable; the full extension set (redis, mongodb,
  imagick, parallel, memcached, …) is unchanged.
- **`download_binary` now accepts FrankenPHP's version format.** The version
  validator rejected the `{php}-{frankenphp}` scheme every FrankenPHP release
  uses (e.g. `8.5-1.12.4`), so the exact string from `get_available_versions`
  couldn't be installed via the API/MCP. It now round-trips correctly.
- **FrankenPHP versions are labelled by PHP line.** `get_available_versions`
  had labelled all three builds "PHP 8.4"; they now read 8.3 / 8.4 / 8.5.
- **`burd start`/`restart` verify the service actually came up.** They now wait
  for the instance port to accept connections before reporting success, instead
  of claiming success for a process that died on startup — and point at
  `burd logs` when it doesn't.
- **`burd doctor` is honest about dead services.** The coverage summary now
  reports `configured but not running` for a service whose process isn't up,
  rather than a misleading `[OK]`.

---

## [1.10.0] - 2026-07-20

### Added

- **Manage PostgreSQL extensions from the GUI.** Running PostgreSQL instances
  now have an Extensions button that opens a per-database manager to enable or
  disable pgvector, pg_partman, and the bundled contrib extensions with a
  single toggle (`CREATE`/`DROP EXTENSION` under the hood).
- **PostgreSQL extensions over the HTTP API and MCP.** New tools
  `list_database_extensions`, `enable_database_extension`, and
  `disable_database_extension` let AI agents turn on pgvector (for vector
  search) and other extensions without leaving the assistant.
- **Manage the CLI PHP version over the HTTP API and MCP.** New tools
  `list_php_cli_versions`, `list_available_php_cli_versions`,
  `install_php_cli_version`, `uninstall_php_cli_version`,
  `switch_php_cli_version`, `get_php_cli_status`, and `configure_php_cli_shell`
  expose the nvm-style PHP version manager that was previously GUI-only.

### Changed

- **MCP surface repositioned as a Docker alternative for AI harnesses.** The
  usage guide now leads with a Docker-alternative overview, don't/do pairs, and
  a concept map (`create_instance` ≈ `docker run`, `list_instances` ≈
  `docker ps`, `create_stack`/`start_stack` ≈ `docker-compose up`,
  `get_instance_env` ≈ a container's published ports/env). Individual tool
  descriptions gained Docker analogies and next-step hints so agents reach for
  Burd instead of `docker run`, docker-compose, or bare `redis-server` /
  `php artisan serve`.
- **Slimmer in-app updater UI.** Dropped the top header update banner; the
  sidebar badge is now the single update affordance.

---

## [1.9.3] - 2026-07-19

### Added

- **Switch the active CLI PHP version from the menubar** — a "PHP Version"
  submenu lists installed versions and switches the global default in one
  click (Herd-like).
- **Custom PHP builds with a full extension set.** FrankenPHP (web) and the
  PHP CLI now come from Burd's own builds (PHP 8.3/8.4/8.5) carrying redis,
  mongodb, imagick, intl, ffi, xlswriter, memcached, and ~60 more — the
  extensions the official builds omit. FrankenPHP versions read as
  `<php>-<frankenphp>` (e.g. `8.4-1.12.4`) so the PHP line is visible.
- **PostgreSQL ships pgvector and pg_partman** (plus the 60 bundled contrib
  extensions), available via `CREATE EXTENSION`.

### Changed

- The PHP version manager downloads CLI binaries from Burd's own release
  channel, verifies each against its published checksum, and code-signs it
  on install.

### Fixed

- Reverse-proxy status is read from launchd (not an unprivileged `lsof`),
  so the app no longer mistakes its own running root daemon for a port
  conflict on reopen.

---

## [1.9.2] - 2026-07-17

### Added

- The updater now checks for new versions every 30 minutes (not only at
  launch), and shows a small clickable badge next to the version in the
  sidebar when an update is available — click it to install and restart.

---

## [1.9.1] - 2026-07-17

### Changed

- Maintenance release. First build published after the auto-updater, so
  existing 1.9.0 installs can verify the in-app update flow.

---

## [1.9.0] - 2026-07-17

### Added

- **In-app auto-updater.** On launch, Burd checks GitHub Releases for a
  newer signed build and offers to install it and relaunch, with a
  progress bar. Updates are cryptographically verified (minisign) before
  install. Apple Silicon only for now; Intel continues to update via the
  DMG or Homebrew.

---

## [1.8.2] - 2026-07-17

### Fixed

- The reverse-proxy daemon status is now read correctly: the app queries
  `launchctl` for the root daemon's PID (an unprivileged `lsof` couldn't
  see it, so a healthy proxy looked down), and the health check probes the
  configured TLD instead of a hardcoded one. This stops the app from
  trying to reinstall/restart an already-running proxy and colliding on
  ports 80/443.
- CLI release binaries are now ad-hoc code-signed, so they run when
  downloaded directly or via `burd upgrade` (Apple Silicon rejects
  unsigned binaries).

---

## [1.8.1] - 2026-07-17

### Changed

- Service binaries (mariadb, mysql, postgresql, redis, valkey, memcached,
  beanstalkd, frpc, mailpit) are now downloaded from the
  [`burd-binaries`](https://github.com/digitalnodecom/burd-binaries) GitHub
  releases with a pinned SHA-256 that is verified before install, replacing
  the previous object-storage host. Bumps these services to current upstream
  versions.

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
