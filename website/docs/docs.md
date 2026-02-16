# Burd Documentation

Welcome to Burd, the ultimate local development environment for PHP developers. This documentation covers everything you need to know to get started and make the most of Burd's powerful features.

---

# Getting Started

## What is Burd?

Burd is a native macOS application that provides a complete local development environment for PHP developers. It bundles 18+ integrated services including databases, caching, search, email testing, and more - all without Docker or complex configuration.

**Key Features:**
- **18+ Integrated Services** - MariaDB, PostgreSQL, MongoDB, Redis, Meilisearch, Mailpit, and more
- **Zero Docker Complexity** - Native binaries for maximum performance
- **Park Directories** - Automatic project detection like Laravel Valet
- **Built-in Tunneling** - Share local sites publicly in seconds
- **Powerful CLI** - Full feature parity from the command line
- **21+ Project Types** - Automatic detection for Laravel, WordPress, Symfony, and more

## System Requirements

- **Operating System:** macOS 12.0 (Monterey) or later
- **Architecture:** Apple Silicon (M1/M2/M3) or Intel
- **Disk Space:** 2GB minimum for app and services
- **Memory:** 8GB RAM recommended

> Windows and Linux versions are coming soon. Lifetime license holders will get access to all platforms at no extra cost.

## Installation

### Download Burd

1. Visit [burd.dev](https://burd.dev) to download the latest version
2. Open the `.dmg` file and drag Burd to your Applications folder
3. Launch Burd from Applications

### Install the CLI

The Burd CLI provides command-line access to all features. Install it by running:

```bash
# The CLI is bundled with the app
# Add to your PATH:
export PATH="$PATH:/Applications/Burd.app/Contents/MacOS"

# Or create a symlink:
sudo ln -s /Applications/Burd.app/Contents/MacOS/burd /usr/local/bin/burd
```

Verify the installation:

```bash
burd --version
```

## Quick Start

### 1. Create Your First Project

```bash
# Create a new Laravel project
burd new laravel myapp

# Or WordPress
burd new wordpress myblog

# Or Bedrock (modern WordPress)
burd new bedrock mysite
```

### 2. Link to a Domain

```bash
cd myapp
burd link
# Your site is now available at http://myapp.burd
```

### 3. Open in Browser

```bash
burd open
# Opens http://myapp.burd in your default browser
```

### 4. Full Project Setup (Recommended)

For a complete setup with database, cache, and mail:

```bash
cd myapp
burd setup
```

This interactive wizard will:
- Create a FrankenPHP instance and domain
- Set up a database
- Configure Redis for caching/sessions
- Configure Mailpit for email testing
- Run database migrations (Laravel)

---

# Core Concepts

## Instances & Services

Burd manages **instances** of various **services**. Each instance is an independent copy of a service that you can start, stop, and configure.

**Services** are the underlying software:
- MariaDB, PostgreSQL, MongoDB (databases)
- Redis, Valkey, Memcached (caching)
- Meilisearch, Typesense (search)
- And more...

**Instances** are running copies you create:
- "My MariaDB" on port 3330
- "Project Redis" on port 6379
- Multiple instances of the same service on different ports

## Domains & TLD

Burd uses a custom top-level domain (TLD) for local development. By default, this is `.burd`.

When you link a project, it becomes accessible at `projectname.burd`:

```bash
cd ~/Projects/myapp
burd link
# Available at: http://myapp.burd
```

You can also specify a custom subdomain:

```bash
burd link api
# Available at: http://api.burd
```

### HTTPS Support

Enable HTTPS for any domain:

```bash
burd secure myapp
# Now available at: https://myapp.burd
```

Burd uses mkcert to generate locally-trusted certificates. Your browser will show a valid certificate without warnings.

## Park Directories

Park directories provide automatic project discovery, similar to Laravel Valet. Simply "park" a folder, and all subdirectories automatically become accessible domains.

```bash
cd ~/Projects
burd park

# Now any folder in ~/Projects is accessible:
# ~/Projects/blog    -> http://blog.burd
# ~/Projects/api     -> http://api.burd
# ~/Projects/shop    -> http://shop.burd
```

Burd automatically detects the project type and configures the correct document root.

## Project Detection

Burd recognizes 21+ project types automatically:

**Frameworks:**
- Laravel, Symfony, CakePHP, Slim

**CMS Platforms:**
- WordPress, Bedrock, Statamic, Craft CMS
- Drupal, Magento, Joomla, Kirby
- OctoberCMS, ConcreteCMS, Contao, ExpressionEngine

**Static Site Generators:**
- Jigsaw, Sculpin, Katana

**Generic:**
- PHP projects with index.php
- Static HTML sites

Each project type maps to the correct document root:
- Laravel/Symfony: `public/`
- Bedrock/Craft: `web/`
- WordPress: project root
- And more...

---

# Services Reference

## Databases

### MariaDB

MySQL-compatible database server.

| Setting | Value |
|---------|-------|
| Default Port | 3330 |
| Username | root |
| Password | (empty) |
| Socket | `/tmp/mariadb-{instance-id}.sock` |

**Connection Example (Laravel .env):**

```env
DB_CONNECTION=mysql
DB_HOST=127.0.0.1
DB_PORT=3330
DB_DATABASE=myapp
DB_USERNAME=root
DB_PASSWORD=
```

**Requirements:** Install via Homebrew: `brew install mariadb`

### PostgreSQL

Advanced open-source relational database.

| Setting | Value |
|---------|-------|
| Default Port | 5432 |
| Username | your system username |
| Password | (none required locally) |

**Connection Example (Laravel .env):**

```env
DB_CONNECTION=pgsql
DB_HOST=127.0.0.1
DB_PORT=5432
DB_DATABASE=myapp
DB_USERNAME=yourusername
DB_PASSWORD=
```

**Requirements:** Install via Homebrew: `brew install postgresql@17`

### MongoDB

NoSQL document database.

| Setting | Value |
|---------|-------|
| Default Port | 27017 |
| Versions | 8.0, 7.0, 6.0 |

**Connection Example:**

```env
MONGODB_URI=mongodb://127.0.0.1:27017/myapp
```

## Cache & Sessions

### Redis

In-memory data store for caching, sessions, and queues.

| Setting | Value |
|---------|-------|
| Default Port | 6379 |
| Password | Optional (configurable) |

**Laravel Configuration:**

```env
CACHE_STORE=redis
SESSION_DRIVER=redis
REDIS_HOST=127.0.0.1
REDIS_PORT=6379
REDIS_PASSWORD=null
```

### Valkey

Open-source Redis fork with full compatibility.

| Setting | Value |
|---------|-------|
| Default Port | 6380 |
| Password | Optional (configurable) |

Use the same configuration as Redis, just change the port.

### Memcached

High-performance distributed memory caching.

| Setting | Value |
|---------|-------|
| Default Port | 11211 |
| Memory | 64MB (default) |

**Laravel Configuration:**

```env
CACHE_STORE=memcached
MEMCACHED_HOST=127.0.0.1
MEMCACHED_PORT=11211
```

## Search Engines

### Meilisearch

Lightning-fast search engine perfect for Laravel Scout.

| Setting | Value |
|---------|-------|
| Default Port | 7700 |
| Master Key | Configurable |
| Health Check | `http://127.0.0.1:7700/health` |

**Laravel Scout Configuration:**

```env
SCOUT_DRIVER=meilisearch
MEILISEARCH_HOST=http://127.0.0.1:7700
MEILISEARCH_KEY=your-master-key
```

### Typesense

Fast, typo-tolerant search engine with vector search support.

| Setting | Value |
|---------|-------|
| Default Port | 8108 |
| API Key | xyz (default) |
| Health Check | `http://127.0.0.1:8108/health` |

**Configuration:**

```env
TYPESENSE_HOST=127.0.0.1
TYPESENSE_PORT=8108
TYPESENSE_API_KEY=xyz
```

## Queue & Real-time

### Beanstalkd

Simple, fast work queue.

| Setting | Value |
|---------|-------|
| Default Port | 11300 |

**Laravel Configuration:**

```env
QUEUE_CONNECTION=beanstalkd
BEANSTALKD_HOST=127.0.0.1
BEANSTALKD_PORT=11300
```

### Centrifugo

Real-time messaging server for WebSocket connections.

| Setting | Value |
|---------|-------|
| Default Port | 8000 |
| API Key | Configurable |
| Admin UI | Optional |

**Configuration:**

```env
CENTRIFUGO_URL=http://127.0.0.1:8000
CENTRIFUGO_API_KEY=your-api-key
CENTRIFUGO_SECRET=your-hmac-secret
```

## Storage

### MinIO

S3-compatible object storage for local development.

| Setting | Value |
|---------|-------|
| API Port | 9000 |
| Console Port | 9001 |
| Root User | minioadmin |
| Root Password | minioadmin |

**Laravel S3 Configuration:**

```env
AWS_ACCESS_KEY_ID=minioadmin
AWS_SECRET_ACCESS_KEY=minioadmin
AWS_DEFAULT_REGION=us-east-1
AWS_BUCKET=your-bucket
AWS_ENDPOINT=http://127.0.0.1:9000
AWS_USE_PATH_STYLE_ENDPOINT=true
```

Access the MinIO Console at `http://127.0.0.1:9001`

## Email

### Mailpit

Email testing tool that captures all outgoing mail.

| Setting | Value |
|---------|-------|
| Web UI Port | 8025 |
| SMTP Port | 1025 |

**Laravel Configuration:**

```env
MAIL_MAILER=smtp
MAIL_HOST=127.0.0.1
MAIL_PORT=1025
MAIL_USERNAME=null
MAIL_PASSWORD=null
MAIL_ENCRYPTION=null
```

Access the Mailpit inbox at `http://127.0.0.1:8025`

## PHP

### FrankenPHP

Modern PHP application server built on Caddy.

| Setting | Value |
|---------|-------|
| Default Port | 8000+ |
| PHP Versions | 7.4 - 8.4 |

**Configurable PHP Settings:**
- `php_memory_limit` - Memory limit (default: 256M)
- `php_upload_max_filesize` - Upload limit (default: 64M)
- `php_post_max_size` - POST limit (default: 64M)
- `php_max_execution_time` - Timeout (default: 30s)

### FrankenPHP Park

Special singleton instance that serves all parked directories.

| Setting | Value |
|---------|-------|
| Default Port | 8888 |

This instance is automatically created when you use park directories.

## Automation

### Node-RED

Visual flow-based programming for automation.

| Setting | Value |
|---------|-------|
| Default Port | 1880 |

Access the Node-RED editor at `http://127.0.0.1:1880`

**Requirements:** Node.js via NVM

---

# CLI Reference

The Burd CLI provides complete access to all features from your terminal.

## Project Commands

### burd new

Create a new project from a template.

```bash
burd new <type> <name>
```

**Types:**
- `laravel` - Laravel PHP framework
- `wordpress` or `wp` - Standard WordPress
- `bedrock` - Roots Bedrock (modern WordPress)

**Examples:**

```bash
burd new laravel myapp
burd new wordpress blog
burd new bedrock client-site
```

### burd init

Initialize the current directory with a FrankenPHP instance and domain.

```bash
burd init
```

Creates an instance pointing to the current directory with a domain based on the folder name.

### burd setup

Full interactive project setup wizard. Recommended for new projects.

```bash
burd setup
```

This wizard handles:
1. FrankenPHP instance and domain creation
2. Database setup
3. Redis configuration (Laravel)
4. Mailpit configuration (Laravel)
5. Database migrations (Laravel)

### burd analyze

Analyze the current project to detect type, configuration, and potential issues.

```bash
burd analyze
```

Shows:
- Project type and version
- Document root
- Database configuration
- Cache/Redis settings
- Mail configuration
- Issues and suggestions

### burd doctor

Run a health check on all Burd services and the current project.

```bash
burd doctor
```

Checks:
- All service instances (running/stopped)
- Service coverage (PHP, database, cache, mail, search)
- Proxy installation status
- Current project configuration
- Database connectivity

## Domain Commands

### burd link

Link the current directory to a custom domain.

```bash
burd link [name]
```

**Arguments:**
- `name` - Optional subdomain (defaults to directory name)

**Examples:**

```bash
burd link           # Uses directory name
burd link myapi     # Creates myapi.burd
burd link myapi.burd  # Same as above (TLD stripped)
```

This command also:
- Detects project type and document root
- Offers to create a database
- Checks and fixes .env configuration

### burd unlink

Remove the link for the current directory.

```bash
burd unlink
```

Removes the domain and FrankenPHP instance.

### burd links

List all linked sites.

```bash
burd links
```

**Output:**

```
/path/to/project -> domain.burd (port 8000, HTTP)
/path/to/other   -> other.burd (port 8001, HTTPS)
```

### burd secure

Enable HTTPS for a domain.

```bash
burd secure [name]
```

**Examples:**

```bash
burd secure           # Current directory's domain
burd secure myapp     # Specific domain
```

### burd unsecure

Disable HTTPS for a domain.

```bash
burd unsecure [name]
```

### burd open

Open a site in your default browser.

```bash
burd open [name]
```

**Examples:**

```bash
burd open           # Current directory's domain
burd open myapp     # Specific domain
```

## Proxy Commands

### burd proxy

Create a domain that proxies to a local port (without FrankenPHP).

```bash
burd proxy <name> <port>
```

**Examples:**

```bash
burd proxy myapi 3000      # Proxy myapi.burd to localhost:3000
burd proxy node-app 8080   # Proxy node-app.burd to localhost:8080
```

Use this for Node.js, Go, or other non-PHP services.

### burd unproxy

Remove a proxied domain.

```bash
burd unproxy <name>
```

### burd proxies

List all proxied domains.

```bash
burd proxies
```

## Park Commands

### burd park

Park the current directory for automatic project discovery.

```bash
burd park
```

All subdirectories become accessible as domains automatically.

**Note:** Requires a FrankenPHP Park instance to be created in the Burd app first.

### burd forget

Unpark the current directory.

```bash
burd forget
```

Removes the directory from the parked list and deletes associated domains.

### burd parked

List all parked directories.

```bash
burd parked
```

**Output:**

```
/Users/you/Projects (12 projects, HTTP)
  blog       -> blog.burd (WordPress)
  api        -> api.burd (Laravel)
  docs       -> docs.burd (Static)
```

### burd refresh

Rescan parked directories for new or removed projects.

```bash
burd refresh
```

### burd status

Show park/link status for the current directory.

```bash
burd status
```

Shows whether the directory is:
- Linked (and to which domain)
- Inside a parked directory
- A parked directory itself
- Not configured

## Database Commands

### burd db list

List all databases on configured instances.

```bash
burd db list
```

### burd db create

Create a new database.

```bash
burd db create <name>
```

**Example:**

```bash
burd db create myapp
```

### burd db drop

Drop a database.

```bash
burd db drop <name> [--force]
```

**Options:**
- `--force`, `-f` - Skip confirmation prompt

### burd db import

Import a SQL file into a database.

```bash
burd db import <name> <file>
```

**Example:**

```bash
burd db import myapp backup.sql
```

### burd db export

Export a database to a SQL file.

```bash
burd db export <name> [--output <file>]
```

**Options:**
- `--output`, `-o` - Output file (default: `<name>.sql`)

**Example:**

```bash
burd db export myapp
burd db export myapp --output ~/backups/myapp-2024.sql
```

### burd db shell

Open an interactive database shell.

```bash
burd db shell [name]
```

**Examples:**

```bash
burd db shell           # Connect to default instance
burd db shell myapp     # Connect to myapp database
```

Type `exit` or press Ctrl+D to quit.

## Environment Commands

### burd env check

Validate .env file against Burd services.

```bash
burd env check
```

Checks for mismatches between your .env and running Burd services.

### burd env fix

Interactively fix .env configuration issues.

```bash
burd env fix
```

Prompts for each issue with suggested fixes.

### burd env show

Display relevant .env values.

```bash
burd env show
```

Shows database, cache, mail, and search configuration with sensitive values masked.

## Sharing Commands

### burd share

Share a local site via tunnel.

```bash
burd share [--subdomain <name>]
```

**Options:**
- `--subdomain`, `-s` - Custom subdomain (random if not specified)

**Example:**

```bash
burd share
burd share --subdomain myproject
```

**Requirements:**
- frpc instance created and running in Burd app
- Project must be linked or inside a parked directory

## Maintenance Commands

### burd upgrade

Update the Burd CLI to the latest version.

```bash
burd upgrade [--check]
```

**Options:**
- `--check`, `-c` - Only check for updates, don't install

---

# Features Guide

## Park Directories

Park directories provide automatic project discovery and domain routing, similar to Laravel Valet.

### How It Works

1. **Park a folder** containing your projects
2. **Burd scans** for project subdirectories
3. **Each project** gets a domain automatically
4. **Access any project** at `projectname.burd`

### Setting Up Parks

First, ensure you have a FrankenPHP Park instance in the Burd app, then:

```bash
cd ~/Projects
burd park
```

Now every folder in `~/Projects` is accessible:

```
~/Projects/
├── blog/        -> http://blog.burd
├── api/         -> http://api.burd
├── client-site/ -> http://client-site.burd
└── experiments/ -> http://experiments.burd
```

### Project Detection

Burd detects 21+ project types and configures the correct document root:

| Project Type | Detection | Document Root |
|--------------|-----------|---------------|
| Laravel | `artisan` file | `public/` |
| Statamic | `artisan` + `content/` | `public/` |
| WordPress | `wp-config.php` | root |
| Bedrock | `web/wp/` structure | `web/` |
| Symfony | `bin/console` | `public/` |
| Craft CMS | `craft` executable | `web/` |
| Drupal | `core/lib/Drupal.php` | root |
| Static HTML | `index.html` | root or `public/` |

### Managing Parks

```bash
# List all parked directories
burd parked

# Rescan for new projects
burd refresh

# Check current directory status
burd status

# Remove a park
burd forget
```

## Custom Drivers

Extend project detection with custom TOML-based drivers.

### Driver Locations

1. **Local Driver** (highest priority): `{project}/.burd/driver.toml`
2. **Global Drivers**: `~/.config/burd/drivers/*.toml`
3. **Built-in Detection**: Hardcoded rules (lowest priority)

### TOML Format

```toml
[driver]
name = "My Framework"
priority = 50        # Lower = checked first (default: 100)
requires_php = true

[detection]
# Files that must exist
files_exist = [
    "artisan",
    "composer.json"
]

# Directories that must exist
dirs_exist = [
    "app",
    "config"
]

# Files that must NOT exist (exclude similar frameworks)
files_not_exist = [
    "web/wp/wp-settings.php"
]

# File content patterns (regex supported)
file_contains = { "composer.json" = "my-framework/core" }

[document_root]
path = "public"
fallbacks = ["www", "."]
```

### Example: Custom CMS Driver

Create `~/.config/burd/drivers/mycms.toml`:

```toml
[driver]
name = "MyCMS"
priority = 25
requires_php = true

[detection]
files_exist = ["core/bootstrap.php", "config/app.php"]
dirs_exist = ["app/Controllers"]

[document_root]
path = "public"
fallbacks = ["."]
```

### Local Project Override

Create `.burd/driver.toml` in any project to override detection:

```toml
[driver]
name = "Custom Setup"
requires_php = true

[document_root]
path = "dist"
```

## Tinker Console

Interactive PHP REPL with framework integration.

### Supported Frameworks

- **Laravel**: Uses `php artisan tinker`
- **WordPress**: Loads `wp-load.php` context
- **Bedrock**: Loads Bedrock's WordPress context
- **Generic PHP**: Executes PHP directly

### Using Tinker

The Tinker console is available in the Burd app. It provides:

- Syntax highlighting
- Execution history
- Framework context (access models, helpers, etc.)
- Multiple PHP version support

### Laravel Example

```php
// Access Eloquent models
User::where('email', 'like', '%@example.com')->count()

// Use helpers
route('home')

// Query the database
DB::table('users')->get()
```

### WordPress Example

```php
// Access WordPress functions
get_bloginfo('name')

// Query posts
$posts = get_posts(['numberposts' => 5]);

// Use WP_Query
$query = new WP_Query(['post_type' => 'page']);
```

## Tunneling & Sharing

Share local sites publicly using built-in frp tunneling.

### Prerequisites

1. Create an **frpc (Tunnels)** instance in Burd app
2. Configure a tunnel server
3. Start the frpc instance

### Sharing a Site

```bash
cd ~/Projects/myapp
burd share
```

This creates a public URL like `https://abc123.tunnel.example.com`

### Custom Subdomain

```bash
burd share --subdomain myproject
# Creates: https://myproject.tunnel.example.com
```

### How It Works

1. frpc connects to your configured frp server
2. A tunnel is created for your local site
3. Traffic flows: Internet -> frp server -> frpc -> Burd -> Your site

### Managing Tunnels

Tunnels are managed in the Burd app:
- Create/delete tunnels
- Set custom subdomains
- Configure auto-start
- Monitor connection status

## Project Analysis

Burd can analyze projects and detect configuration issues.

### Running Analysis

```bash
burd analyze
```

### What Gets Analyzed

- **Project Type**: Laravel, WordPress, Symfony, etc.
- **Document Root**: Correct path for web server
- **Database Config**: Connection settings and port matching
- **Cache Config**: Redis/Memcached settings
- **Mail Config**: SMTP settings for Mailpit
- **Search Config**: Meilisearch/Typesense settings

### Issue Detection

The analyzer checks your project against running Burd services:

```
[!] Database port mismatch: .env has 3306, Burd MariaDB is on 3330
    Suggestion: Update DB_PORT to 3330

[*] Using Mailtrap but Burd has Mailpit available
    Suggestion: Switch to Mailpit for local mail testing

[i] Meilisearch not configured
    Suggestion: Create a Meilisearch instance for fast search
```

### Using with Burd Services

```bash
# Check project configuration
burd env check

# Interactively fix issues
burd env fix

# View current settings
burd env show
```

---

# Framework Guides

## Laravel

### Quick Setup

```bash
# Create new Laravel project
burd new laravel myapp
cd myapp

# Full setup wizard
burd setup
```

### Manual Setup

```bash
# Create project
composer create-project laravel/laravel myapp
cd myapp

# Link to domain
burd link

# Create database
burd db create myapp
```

### Environment Configuration

Update your `.env` file:

```env
APP_URL=http://myapp.burd

DB_CONNECTION=mysql
DB_HOST=127.0.0.1
DB_PORT=3330
DB_DATABASE=myapp
DB_USERNAME=root
DB_PASSWORD=

CACHE_STORE=redis
SESSION_DRIVER=redis
REDIS_HOST=127.0.0.1
REDIS_PORT=6379

MAIL_MAILER=smtp
MAIL_HOST=127.0.0.1
MAIL_PORT=1025

SCOUT_DRIVER=meilisearch
MEILISEARCH_HOST=http://127.0.0.1:7700
```

### Running Migrations

```bash
php artisan migrate
```

Or use the setup wizard which handles this automatically.

## WordPress

### Quick Setup

```bash
# Create new WordPress site
burd new wordpress myblog
cd myblog

# Link to domain
burd link
```

### Database Configuration

The `burd new wordpress` command creates `wp-config.php` with:

```php
define('DB_NAME', 'myblog');
define('DB_USER', 'root');
define('DB_PASSWORD', '');
define('DB_HOST', '127.0.0.1:3330');
```

### Creating the Database

```bash
burd db create myblog
```

Then visit `http://myblog.burd` to complete WordPress installation.

## Bedrock

Bedrock is a modern WordPress boilerplate with better structure.

### Quick Setup

```bash
# Create new Bedrock site
burd new bedrock mysite
cd mysite

# Link to domain
burd link
```

### Environment Configuration

Edit `.env`:

```env
WP_ENV=development
WP_HOME=http://mysite.burd
WP_SITEURL=${WP_HOME}/wp

DB_NAME=mysite
DB_USER=root
DB_PASSWORD=
DB_HOST=127.0.0.1:3330
```

### Directory Structure

```
mysite/
├── config/           # WordPress configuration
├── web/              # Document root
│   ├── app/          # WordPress content (themes, plugins)
│   └── wp/           # WordPress core
├── vendor/           # Composer packages
└── .env              # Environment config
```

## Symfony

### Setup

```bash
# Create Symfony project
composer create-project symfony/skeleton myapp
cd myapp

# Link to domain
burd link
```

### Database Configuration

Edit `.env`:

```env
DATABASE_URL="mysql://root:@127.0.0.1:3330/myapp?serverVersion=10.11.0-MariaDB"
```

For PostgreSQL:

```env
DATABASE_URL="postgresql://username:@127.0.0.1:5432/myapp?serverVersion=17"
```

---

# Configuration

## App Settings

### TLD (Top-Level Domain)

The default TLD is `.burd`. All your local domains use this suffix:
- `myapp.burd`
- `api.burd`
- `blog.burd`

### Auto-Start Services

Configure services to start automatically when Burd launches. Set this per-instance in the Burd app.

## Service Configuration

### PHP Settings (FrankenPHP)

Configure PHP settings per instance:

| Setting | Default | Description |
|---------|---------|-------------|
| `php_memory_limit` | 256M | Maximum memory per script |
| `php_upload_max_filesize` | 64M | Maximum upload file size |
| `php_post_max_size` | 64M | Maximum POST data size |
| `php_max_execution_time` | 30 | Script timeout (seconds) |

### Database Credentials

**MariaDB:**
- Username: `root`
- Password: (empty by default)
- Configure password in instance settings

**PostgreSQL:**
- Uses system authentication
- Username: your macOS username

### API Keys

Services that support API keys:
- **Meilisearch**: Master key for API authentication
- **Typesense**: API key for requests
- **Centrifugo**: API key and HMAC secret
- **MinIO**: Access key and secret key

---

# Troubleshooting

## Common Issues

### Port Conflicts

**Symptom:** Service won't start, "port already in use" error

**Solution:**

```bash
# Find what's using the port
lsof -i :3330

# Kill the process or change the Burd instance port
```

### Service Not Starting

**Symptom:** Instance shows as stopped, won't start

**Steps:**

1. Check the instance logs in Burd app
2. Verify requirements are installed:
   ```bash
   # For MariaDB
   brew install mariadb

   # For PostgreSQL
   brew install postgresql@17
   ```
3. Run `burd doctor` to check service status

### HTTPS Not Working

**Symptom:** Browser shows certificate error

**Solution:**

1. Ensure Caddy proxy is installed (check in Burd app)
2. Trust the local CA:
   ```bash
   # mkcert is bundled with Burd
   mkcert -install
   ```
3. Re-enable HTTPS:
   ```bash
   burd unsecure myapp
   burd secure myapp
   ```

### Database Connection Issues

**Symptom:** "Connection refused" or wrong port errors

**Check:**

```bash
# Verify Burd database port
burd doctor

# Check your .env
burd env check

# Fix automatically
burd env fix
```

**Common fixes:**
- Change `DB_PORT` from `3306` to `3330` (Burd's MariaDB default)
- Ensure the database instance is running
- Verify database exists: `burd db list`

### .env Mismatches

**Symptom:** Project can't connect to services

**Solution:**

```bash
# Check for issues
burd env check

# Fix interactively
burd env fix

# View current config
burd env show
```

## Diagnostics

### Using burd doctor

The `burd doctor` command is your first stop for troubleshooting:

```bash
burd doctor
```

**Output example:**

```
Services
--------
[OK]  MariaDB (port 3330)
[OK]  Redis (port 6379)
[ERR] Meilisearch - not responding

Service Coverage
----------------
[OK] PHP Server
[OK] Database
[OK] Cache
[--] Search (not configured)

Current Project: /Users/you/myapp
---------------------------------
Type: Laravel
[OK]  Linked to myapp.burd
[OK]  Database 'myapp' exists
[WARN] Cache not configured for Redis
```

### Checking Logs

View instance logs in the Burd app by clicking on an instance and selecting "Logs".

### Service Health Checks

Each service has a health check:

| Service | Health Check |
|---------|--------------|
| MariaDB | TCP connection |
| PostgreSQL | TCP connection |
| Redis | TCP connection |
| Meilisearch | HTTP `/health` |
| Mailpit | HTTP `/livez` |
| MinIO | HTTP `/minio/health/live` |

---

# Advanced

## MCP Integration

Burd includes an MCP (Model Context Protocol) server for AI agent integration.

### Configuration

Add to your Claude Desktop config:

```json
{
  "mcpServers": {
    "burd": {
      "command": "/usr/local/bin/burd",
      "args": ["mcp"]
    }
  }
}
```

### Usage

The MCP server allows AI agents to:
- Create and manage instances
- Link/unlink projects
- Manage databases
- Control services
- Query project status

## Multiple PHP Versions

Burd includes a PHP Version Manager for running different PHP versions.

### Available Versions

PHP 7.4 through 8.4 are supported via FrankenPHP.

### Per-Project PHP

Configure different PHP versions for different instances in the Burd app.

## SSL/HTTPS

### How It Works

1. **mkcert** generates locally-trusted certificates
2. **Caddy proxy** handles HTTPS termination
3. **FrankenPHP** receives plain HTTP internally

### Certificate Trust

Burd automatically installs the local CA. If you see certificate warnings:

```bash
# Reinstall the CA
mkcert -install
```

### Forcing HTTPS

Enable HTTPS for a domain:

```bash
burd secure myapp
```

Your Laravel app will automatically detect HTTPS via the `X-Forwarded-Proto` header.

---

# Support

## Getting Help

- **Documentation**: You're reading it!
- **GitHub Issues**: Report bugs and request features
- **Discord**: Community support and discussion

## Reporting Issues

When reporting issues, include:

1. Burd version (`burd --version`)
2. macOS version
3. Output of `burd doctor`
4. Relevant error messages
5. Steps to reproduce

---

*Burd - The Ultimate Local Dev Environment for PHP Developers*
