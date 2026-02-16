# CLI Command Comparison: Burd vs Herd vs Valet

A comprehensive comparison of command-line interfaces for local PHP development environments.

## Command Overview

### Site & Directory Management

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `init` | `burd init` | `herd init` | - | Initialize project in current directory |
| `park` | `burd park` | `herd park` | `valet park` | Park directory for auto-discovery |
| `forget/unpark` | `burd forget` | - | `valet forget` | Remove directory from parked list |
| `parked/paths` | `burd parked` | `herd parked` | `valet paths` | List parked directories |
| `link` | - | `herd link` | `valet link` | Link single site outside parked dirs |
| `unlink` | - | `herd unlink` | `valet unlink` | Remove linked site |
| `links` | - | `herd links` | `valet links` | List all linked sites |
| `refresh` | `burd refresh` | - | - | Refresh parked directory projects |
| `status` | `burd status` | - | - | Show park status for current dir |

### SSL/HTTPS Management

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `secure` | - | `herd secure` | `valet secure` | Enable HTTPS for a site |
| `unsecure` | - | `herd unsecure` | `valet unsecure` | Disable HTTPS for a site |
| `secured` | - | `herd secured` | - | List all secured sites |

### PHP Version Management

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `use` | - | `herd use` | `valet use` | Change global PHP version |
| `php` | - | `herd php` | `valet php` | Run PHP with site-specific version |
| `isolate` | - | `herd isolate` | `valet isolate` | Set PHP version for specific site |
| `isolated` | - | `herd isolated` | `valet isolated` | List sites with isolated PHP |
| `unisolate` | - | `herd unisolate` | `valet unisolate` | Remove PHP version isolation |
| `composer` | - | `herd composer` | `valet composer` | Run Composer with correct PHP |
| `which-php` | - | `herd which-php` | `valet which-php` | Show PHP binary for current site |

### Node.js Version Management

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `isolate-node` | - | `herd isolate-node` | - | Set Node version for directory |
| `isolated-node` | - | `herd isolated-node` | - | List Node-isolated directories |
| `unisolate-node` | - | `herd unisolate-node` | - | Remove Node version isolation |

### Service Management

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `start` | - | `herd start` | `valet start` | Start all services |
| `stop` | - | `herd stop` | `valet stop` | Stop all services |
| `restart` | - | `herd restart` | `valet restart` | Restart all services |

### Site Sharing & Tunnels

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `share` | - | `herd share` | `valet share` | Share site publicly |
| `share-tool` | - | - | `valet share-tool` | Set sharing tool (ngrok/expose) |
| `set-ngrok-token` | - | - | `valet set-ngrok-token` | Configure ngrok token |

### Proxy Management

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `proxy` | - | - | `valet proxy` | Proxy domain to another URL |
| `unproxy` | - | - | `valet unproxy` | Remove proxy configuration |
| `proxies` | - | - | `valet proxies` | List all proxied sites |

### Developer Tools

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `tinker` | - | `herd tinker` | - | Open Tinker session |
| `log` | - | `herd log` | `valet log` | Tail service logs |
| `logs` | - | `herd logs` | - | Open Log Viewer (Pro) |
| `edit` | - | `herd edit` | - | Open project in IDE |
| `open` | - | `herd open` | - | Open project in browser |
| `ini` | - | `herd ini` | - | Open php.ini in IDE |
| `coverage` | - | `herd coverage` | - | Run PHP with Xdebug coverage |
| `debug` | - | `herd debug` | - | Run with debug.ini loaded |

### Information & Diagnostics

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `list` | - | `herd list` | `valet list` | Show all available commands |
| `which` | - | `herd which` | - | Show detected site driver |
| `site-information` | - | `herd site-information` | - | Show site/app information |
| `diagnose` | - | - | `valet diagnose` | Output diagnostics |
| `directory-listing` | - | - | `valet directory-listing` | Configure directory listing |

### Installation & System

| Command | Burd | Herd | Valet | Description |
|---------|:----:|:----:|:-----:|-------------|
| `install` | - | - | `valet install` | Install Valet |
| `uninstall` | - | - | `valet uninstall` | Uninstall Valet |
| `trust` | - | - | `valet trust` | Add sudoers for passwordless |

---

## Current Burd CLI Commands

```
burd init      Create a FrankenPHP instance for current directory
burd park      Park directory (auto-create domains for subdirectories)
burd forget    Unpark the current directory
burd parked    List all parked directories and their projects
burd refresh   Refresh parked directories (scan for changes)
burd status    Show park status for current directory
```

---

## Missing CLI Commands (Priority List)

### High Priority (Core Functionality)

| Command | Purpose | Complexity |
|---------|---------|------------|
| `burd link [name]` | Link single project outside parked dirs | Medium |
| `burd unlink` | Remove linked site | Low |
| `burd links` | List all linked sites | Low |
| `burd secure [domain]` | Enable SSL for a domain | Medium |
| `burd unsecure [domain]` | Disable SSL for a domain | Low |
| `burd start` | Start all/specific services | Medium |
| `burd stop` | Stop all/specific services | Medium |
| `burd restart` | Restart all/specific services | Medium |

### Medium Priority (PHP/Node Management)

| Command | Purpose | Complexity |
|---------|---------|------------|
| `burd php` | Run PHP with correct version | Medium |
| `burd use [version]` | Switch global PHP version | Medium |
| `burd isolate [version]` | Set PHP version per site | High |
| `burd unisolate` | Remove PHP isolation | Medium |
| `burd composer` | Run Composer with correct PHP | Medium |
| `burd which-php` | Show PHP binary for site | Low |

### Lower Priority (Nice to Have)

| Command | Purpose | Complexity |
|---------|---------|------------|
| `burd share` | Share site via tunnel | Medium |
| `burd open` | Open site in browser | Low |
| `burd edit` | Open in IDE | Low |
| `burd log` | Tail service logs | Medium |
| `burd tinker` | Open PHP REPL | Low (already in GUI) |
| `burd list` | Show all commands | Low (clap provides) |
| `burd diagnose` | System diagnostics | Medium |

---

## CLI Feature Parity Summary

| Category | Burd | Herd | Valet |
|----------|------|------|-------|
| **Site Management** | 6 commands | 8 commands | 7 commands |
| **SSL Management** | 0 commands | 3 commands | 2 commands |
| **PHP Management** | 0 commands | 7 commands | 7 commands |
| **Node Management** | 0 commands | 3 commands | 0 commands |
| **Service Control** | 0 commands | 3 commands | 3 commands |
| **Sharing/Tunnels** | 0 commands | 1 command | 3 commands |
| **Developer Tools** | 0 commands | 8 commands | 2 commands |
| **Diagnostics** | 0 commands | 2 commands | 3 commands |
| **Total** | **6** | **35** | **27** |

### CLI Parity Estimate

- **vs Valet**: ~22% (6/27 commands)
- **vs Herd**: ~17% (6/35 commands)

**Note**: Burd's GUI provides many features that aren't yet exposed via CLI. The CLI is focused on the most common developer workflow commands (init, park).

---

## Unique Burd CLI Features

| Command | Description |
|---------|-------------|
| `burd refresh` | Scan parked directories for new/removed projects |
| `burd status` | Show detailed park status for current directory |

These commands don't have direct equivalents in Herd or Valet.

---

## Configuration Files Comparison

| Tool | Config File | Purpose |
|------|-------------|---------|
| Burd | `~/.burd/config.json` | Main configuration |
| Herd | `herd.yml` (per project) | Team config sharing |
| Valet | `~/.config/valet/config.json` | Main configuration |
| Valet | `.valetrc` (per project) | Site-specific settings |
| Valet | `.valet-env.php` (per project) | Environment variables |

---

*Last updated: January 2025*

**Sources:**
- [Laravel Herd CLI Documentation](https://herd.laravel.com/docs/macos/advanced-usage/herd-cli)
- [Laravel Valet Documentation](https://laravel.com/docs/12.x/valet)
