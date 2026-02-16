# Burd CLI Documentation

The Burd CLI (`burd`) provides command-line tools for managing local PHP development environments.

## Installation

The CLI is automatically installed when you set up Burd. You can also manually install it from the Burd app under Settings > CLI.

## Commands Overview

| Command | Description |
|---------|-------------|
| `burd new` | Create a new project (Laravel, WordPress, Bedrock) |
| `burd setup` | Full interactive project setup wizard |
| `burd doctor` | Health check for services and current project |
| `burd upgrade` | Update CLI to latest version |
| `burd analyze` | Analyze current project (detect type, config, issues) |
| `burd init` | Create a development server for current directory |
| `burd link` | Link current directory to a custom domain |
| `burd unlink` | Remove the link for current directory |
| `burd links` | List all linked sites |
| `burd secure` | Enable HTTPS for a domain |
| `burd unsecure` | Disable HTTPS for a domain |
| `burd open` | Open site in default browser |
| `burd proxy` | Proxy a domain to a local port |
| `burd unproxy` | Remove a proxied domain |
| `burd proxies` | List all proxied domains |
| `burd park` | Park current directory (auto-create domains for subdirs) |
| `burd forget` | Unpark current directory |
| `burd parked` | List all parked directories |
| `burd refresh` | Refresh parked directories |
| `burd status` | Show park status for current directory |
| `burd share` | Share a site via tunnel |
| `burd db` | Database management commands |
| `burd env` | Environment file management |

---

## Project Creation

### `burd new <type> <name>`

Creates a new project from a template. Supported project types:

- `laravel` - Laravel PHP framework (uses Composer)
- `wordpress` or `wp` - Standard WordPress installation (downloads from wordpress.org)
- `bedrock` - Roots Bedrock WordPress (uses Composer)

**Examples:**
```bash
# Create a new Laravel project
$ burd new laravel myapp

Creating Laravel project 'myapp'...

Running: composer create-project laravel/laravel myapp

Laravel project created.
Generating application key...

Project created successfully!

Next steps:
  cd myapp
  burd link
```

```bash
# Create a new WordPress project
$ burd new wordpress myblog

Creating WordPress project 'myblog'...

Downloading WordPress...
Extracting...
Creating wp-config.php...

WordPress installed.

Project created successfully!

Next steps:
  cd myblog
  burd link
```

```bash
# Create a new Bedrock project
$ burd new bedrock mysite

Creating Bedrock project 'mysite'...

Running: composer create-project roots/bedrock mysite

Bedrock project created.
Creating .env from .env.example...

Project created successfully!

Next steps:
  cd mysite
  burd link
```

**What it does:**

| Type | Actions |
|------|---------|
| Laravel | Runs `composer create-project laravel/laravel`, generates app key |
| WordPress | Downloads from wordpress.org, extracts, creates wp-config.php with project name as DB |
| Bedrock | Runs `composer create-project roots/bedrock`, creates .env from template |

**Requirements:**
- Laravel/Bedrock: Composer must be installed
- WordPress: curl must be available

---

## Project Setup

### `burd setup`

Full interactive project setup wizard. Analyzes your project and guides you through configuring everything needed for development.

**What it does:**
1. Analyzes project type (Laravel, WordPress, Bedrock, Symfony)
2. Creates FrankenPHP instance and domain
3. Sets up database (creates if needed, fixes .env)
4. Configures Redis for cache/sessions (Laravel)
5. Configures Mailpit for local mail (Laravel)
6. Runs migrations (Laravel, optional)

**Example:**
```bash
$ cd ~/projects/myapp
$ burd setup

Burd Project Setup
==================

Directory: /Users/dev/myapp

Analyzing project...
Detected: Laravel 11

[1/5] FrankenPHP Instance
-------------------------
Create instance 'myapp' on myapp.test? [Y/n] y
Created instance 'myapp' -> myapp.test

[2/5] Database
--------------
Create database 'myapp' on MariaDB (port 3330)? [Y/n] y
Created database 'myapp'.

Update .env to use Burd's MariaDB?
  DB_PORT = 3330
Apply? [Y/n] y
Updated .env

[3/5] Cache (Redis)
-------------------
Configure Redis for cache/sessions (port 6379)? [Y/n] y
Updated CACHE_STORE, SESSION_DRIVER, REDIS_HOST, REDIS_PORT

[4/5] Mail (Mailpit)
--------------------
Configure Mailpit for local mail (SMTP 1025, Web 8025)? [Y/n] y
Updated MAIL_MAILER, MAIL_HOST, MAIL_PORT

[5/5] Migrations
----------------
Run database migrations? [y/N] y
Running: php artisan migrate
Migrations completed.

Setup Complete!
===============

What was done:
  - Created FrankenPHP instance 'myapp'
  - Created database 'myapp'
  - Configured Redis for cache/sessions
  - Configured Mailpit for local mail
  - Ran database migrations

Access your project:
  URL: https://myapp.test
  Mail: http://localhost:8025

Start the server in the Burd app to begin development.
```

---

## Health Check

### `burd doctor`

Diagnoses issues with Burd services and your current project configuration.

**What it checks:**
- All configured service instances (running/not responding)
- Service coverage (what's installed vs what's missing)
- Proxy status (Caddy HTTPS)
- Current project configuration
- Database connectivity and existence
- Cache configuration
- Mail configuration

**Example:**
```bash
$ burd doctor

Burd Health Check
=================

Services
--------
  [OK] FrankenPHP 'myapp' (port 8001) - running
  [OK] MariaDB 'db' (port 3330) - running
  [OK] Redis 'cache' (port 6379) - running
  [OK] Mailpit 'mail' (port 8025) - running
  [ERR] Meilisearch 'search' (port 7700) - not responding

Service Coverage
----------------
  [OK] PHP Server - configured
  [OK] Database (MariaDB) - configured
  [--] Database (PostgreSQL) - not configured
  [OK] Cache (Redis) - configured
  [OK] Mail (Mailpit) - configured
  [--] Search (Meilisearch) - not configured

Proxy
-----
  [OK] Caddy proxy installed (HTTPS on port 443)

Current Project
---------------
  Type: Laravel 11
  Path: /Users/dev/myapp
  Document Root: /Users/dev/myapp/public
  [OK] Linked to myapp.test

Database:
    [OK] Config points to MariaDB on port 3330
    [OK] Database 'myapp' exists

Cache:
    [OK] Using Redis on port 6379

Mail:
    [OK] Using Mailpit (SMTP 1025, Web http://localhost:8025)

Legend: [OK] = Good, [WARN] = Warning, [ERR] = Error, [--] = Not installed
```

**Status indicators:**
- `[OK]` - Everything is good
- `[WARN]` - Configuration mismatch or potential issue
- `[ERR]` - Service not responding or error
- `[--]` - Not installed/configured

**Common fixes suggested:**
- `burd link` - Link project to a domain
- `burd setup` - Full project setup
- `burd env fix` - Fix .env configuration
- `burd db create <name>` - Create missing database

---

## CLI Updates

### `burd upgrade`

Update the burd CLI to the latest version.

**Options:**
- `--check`, `-c` - Only check for updates, don't install

**Examples:**
```bash
# Check for updates
$ burd upgrade --check

Burd CLI v1.0.0

Checking for updates...

New version available: 1.0.0 -> 1.1.0

Release notes:
  - Added burd setup command
  - Added burd doctor command

Run 'burd upgrade' to install the update.
```

```bash
# Install update
$ burd upgrade

Burd CLI v1.0.0

Checking for updates...

New version available: 1.0.0 -> 1.1.0

Release notes:
  - Added burd setup command
  - Added burd doctor command

Install update? [Y/n] y

Downloading...
Downloaded 2345678 bytes
Verifying checksum...
Checksum verified.
Installing update...

Successfully upgraded to version 1.1.0!

Run 'burd --version' to verify.
```

---

## Project Analysis

### `burd analyze`

Analyzes the current directory to detect project type, parse configuration, and check against Burd services.

**Supported Project Types:**
- Laravel (with version detection)
- WordPress
- Bedrock (Roots WordPress)
- Symfony

**Example:**
```bash
$ cd ~/projects/myapp
$ burd analyze

Project Analysis: Laravel 11
========================================
Path: /Users/dev/myapp
Document Root: public
PHP Version: 8.2

Database:
  Connection: mysql
  Host: 127.0.0.1:3306
  Database: myapp
  Username: root
  [*] Database port 3306 doesn't match Burd's MariaDB on port 3330
      -> Update DB_PORT to 3330 in .env

Cache:
  Driver: redis
  Host: 127.0.0.1
  Port: 6379

Mail:
  Mailer: smtp
  Host: 127.0.0.1:1025

Summary: 0 error(s), 1 warning(s), 0 suggestion(s)
```

**What it checks:**
- Project type detection (Laravel, WordPress, Bedrock, Symfony)
- Document root location
- PHP version requirements (from composer.json)
- Database configuration vs Burd instances
- Cache/Redis configuration
- Mail configuration vs Mailpit
- Search/Meilisearch configuration

---

## Site Linking

### `burd link [name]`

Links the current directory to a custom domain, creating a FrankenPHP instance.

**Smart Setup Features:**
- Detects project type (Laravel, WordPress, Bedrock)
- Offers to create database automatically
- Offers to fix .env configuration issues
- Copies .env.example if .env doesn't exist

**Arguments:**
- `name` - Optional subdomain (defaults to directory name). Can include TLD suffix (e.g., `myapp.burd`).

**Examples:**
```bash
# Use directory name as subdomain
$ burd link

# Use custom subdomain
$ burd link api

# TLD suffix is automatically stripped
$ burd link myapp.burd  # Creates myapp.burd
```

**Example output:**
```bash
$ cd ~/projects/myapp
$ burd link

Linking directory: /Users/dev/myapp

Linked 'myapp' to 'myapp.burd'

  URL: https://myapp.burd
  Port: 8001

Detected: Laravel 11

Create database 'myapp' on MariaDB (port 3330)? [Y/n] y
  Created database 'myapp'

Found 1 .env configuration issue(s):

  DB_PORT = 3306 -> 3330
    Burd's MariaDB is on port 3330
  Apply fix? [y/N] y
    Updated DB_PORT

Start the server with:
  Open Burd app and click Start on 'myapp'

Note: Use 'burd unlink' to remove this link.
```

### `burd unlink`

Removes the link for the current directory.

```bash
$ cd ~/projects/myapp
$ burd unlink

Unlinked 'myapp'
```

### `burd links`

Lists all linked sites.

```bash
$ burd links

Linked Sites:

  /Users/dev/myapp -> myapp.burd (port 8001, SSL)
  /Users/dev/api -> api.burd (port 8002, HTTP)
```

---

## SSL Management

### `burd secure [name]`

Enables HTTPS for a domain.

**Arguments:**
- `name` - Optional domain name (defaults to current directory's domain). Can include TLD suffix.

**Examples:**
```bash
# Enable SSL for current directory's domain
$ burd secure

# Enable SSL for specific domain
$ burd secure myapp

# TLD suffix is automatically stripped
$ burd secure myapp.burd
```

**Example output:**
```bash
$ burd secure myapp

Enabled SSL for myapp.burd

  https://myapp.burd
```

### `burd unsecure [name]`

Disables HTTPS for a domain.

**Examples:**
```bash
# Disable SSL for current directory's domain
$ burd unsecure

# Disable SSL for specific domain
$ burd unsecure myapp
```

---

## Opening Sites

### `burd open [name]`

Opens a site in the default browser.

**Arguments:**
- `name` - Optional domain name (defaults to current directory's domain). Can include TLD suffix.

**Examples:**
```bash
# Open current directory's domain
$ burd open

# Open specific domain
$ burd open myapp

# TLD suffix is automatically stripped
$ burd open myapp.burd
```

**Example output:**
```bash
$ burd open myapp

Opening https://myapp.burd in browser...
```

---

## Port Proxying

### `burd proxy <name> <port>`

Creates a domain that proxies to localhost on the specified port. Unlike `link`, this doesn't create a FrankenPHP instance - useful for proxying to Node.js, Go, or other services.

**Arguments:**
- `name` - Domain name (e.g., 'myapi' or 'myapi.burd')
- `port` - Port to proxy to

**Examples:**
```bash
# Proxy to a Node.js app on port 3000
$ burd proxy myapi 3000

# TLD suffix is automatically stripped
$ burd proxy myapi.burd 8080
```

**Example output:**
```bash
$ burd proxy myapi 3000

Created proxy myapi.burd -> localhost:3000

  http://myapi.burd

Enable SSL with: burd secure myapi
```

### `burd unproxy <name>`

Removes a proxied domain created by `burd proxy`.

**Example:**
```bash
$ burd unproxy myapi

Removed proxy myapi.burd
```

### `burd proxies`

Lists all proxied domains (port-based proxies).

**Example:**
```bash
$ burd proxies

Proxy Domains:

  myapi.burd -> localhost:3000 (HTTP)
  backend.burd -> localhost:8080 (SSL)
```

---

## Directory Parking

### `burd park`

Parks the current directory. All subdirectories automatically become domains.

```bash
$ cd ~/Sites
$ burd park

Parked '/Users/dev/Sites'
All subdirectories will now be accessible as <dirname>.test
```

### `burd forget`

Unparks the current directory.

```bash
$ cd ~/Sites
$ burd forget

Unparked '/Users/dev/Sites'
```

### `burd parked`

Lists all parked directories.

```bash
$ burd parked

Parked Directories:

  /Users/dev/Sites (12 projects)
    - blog.test
    - shop.test
    - api.test
    ...
```

### `burd refresh`

Scans parked directories for new or removed projects.

```bash
$ burd refresh

Refreshed parked directories:
  Added: newproject.test
  Removed: oldproject.test
```

### `burd status`

Shows park status for the current directory.

```bash
$ cd ~/Sites/myproject
$ burd status

This directory is inside a parked directory:
  Parent: /Users/dev/Sites
  Domain: myproject.test
```

---

## Sharing (Tunnels)

### `burd share [--subdomain <name>]`

Exposes a local site to the internet via frpc tunnel.

**Prerequisites:**
- frpc instance created in Burd app
- frpc running and connected to server

**Options:**
- `--subdomain`, `-s` - Custom subdomain for the tunnel

**Example:**
```bash
$ cd ~/projects/myapp
$ burd share

Sharing 'myapp' (port 8001)

  Public URL: https://abc123.your-tunnel-domain.com

Note: This tunnel will persist until removed in the Burd app.
```

With custom subdomain:
```bash
$ burd share --subdomain demo
# Creates demo.your-tunnel-domain.com
```

---

## Database Management

### `burd db list`

Lists all databases on Burd's database instances.

```bash
$ burd db list

db (MariaDB at 127.0.0.1:3330)
----------------------------------------
  myapp
  blog
  shop

postgres (PostgreSQL at 127.0.0.1:5432)
----------------------------------------
  analytics
```

### `burd db create <name>`

Creates a new database.

```bash
$ burd db create myapp

Creating database 'myapp'...
Database 'myapp' created successfully.
```

### `burd db drop <name> [--force]`

Drops a database.

**Options:**
- `--force`, `-f` - Skip confirmation prompt

```bash
$ burd db drop myapp

Are you sure you want to drop database 'myapp'? This cannot be undone. [y/N] y
Dropping database 'myapp'...
Database 'myapp' dropped successfully.
```

### `burd db import <name> <file>`

Imports a SQL file into a database.

```bash
$ burd db import myapp backup.sql

Importing backup.sql into 'myapp'...
Import completed successfully.
```

If the database doesn't exist, you'll be prompted to create it:
```bash
$ burd db import newdb data.sql

Database 'newdb' doesn't exist. Create it? [Y/n] y
Creating database 'newdb'...
Importing data.sql into 'newdb'...
Import completed successfully.
```

### `burd db export <name> [--output <file>]`

Exports a database to a SQL file.

**Options:**
- `--output`, `-o` - Output file path (default: `<name>.sql`)

```bash
$ burd db export myapp

Exporting 'myapp' to myapp.sql...
Export completed: myapp.sql
```

With custom output:
```bash
$ burd db export myapp -o backups/myapp-2024.sql
```

### `burd db shell [name]`

Opens an interactive database shell.

```bash
$ burd db shell myapp

Connecting to MariaDB at 127.0.0.1:3330...
Type 'exit' or Ctrl+D to quit.

MariaDB [myapp]>
```

Without a database name, connects to the default instance:
```bash
$ burd db shell

Connecting to MariaDB at 127.0.0.1:3330...
MariaDB [(none)]>
```

---

## Environment Management

### `burd env check`

Compares your project's `.env` file with running Burd services.

```bash
$ cd ~/projects/myapp
$ burd env check

Found 2 issue(s):

[database]
  DB_PORT = 3306 -> 3330
    Burd's MariaDB is running on port 3330

[mail]
  MAIL_PORT = 587 -> 1025
    Burd's Mailpit SMTP is on port 1025

Run 'burd env fix' to fix these issues interactively.
```

**What it checks:**
- Database host/port vs Burd's MariaDB/PostgreSQL
- Redis host/port vs Burd's Redis
- Mail host/port vs Burd's Mailpit

### `burd env fix`

Interactively fixes `.env` issues.

```bash
$ burd env fix

Found 2 issue(s) in .env file:

[database] DB_PORT
  Current:   3306
  Suggested: 3330
  Reason:    Burd's MariaDB is running on port 3330

Apply this fix? [y/N] y
  Updated DB_PORT

[mail] MAIL_PORT
  Current:   587
  Suggested: 1025
  Reason:    Burd's Mailpit SMTP is on port 1025

Apply this fix? [y/N] y
  Updated MAIL_PORT

Fixed 2 of 2 issue(s).
```

### `burd env show`

Displays relevant `.env` values for the current project.

```bash
$ burd env show

Environment: Laravel 11
========================================

Database:
  DB_CONNECTION = mysql
  DB_HOST = 127.0.0.1
  DB_PORT = 3330
  DB_DATABASE = myapp
  DB_USERNAME = root
  DB_PASSWORD = ********

Cache:
  CACHE_STORE = redis
  SESSION_DRIVER = redis
  REDIS_HOST = 127.0.0.1
  REDIS_PORT = 6379

Mail:
  MAIL_MAILER = smtp
  MAIL_HOST = 127.0.0.1
  MAIL_PORT = 1025

Search:
  SCOUT_DRIVER = meilisearch
  MEILISEARCH_HOST = http://127.0.0.1:7700
```

**Note:** Sensitive values (passwords, keys, tokens) are automatically masked.

---

## Typical Workflows

### Setting up a new Laravel project

```bash
# Fastest way - create and setup everything
burd new laravel myapp
cd myapp
burd setup

# Start the server in Burd app, then visit https://myapp.test
```

Or step by step:
```bash
burd new laravel myapp
cd myapp
burd link           # Creates instance and domain
burd db create myapp
burd env fix        # Fix .env to use Burd services
```

### Setting up an existing project

```bash
cd ~/projects/existing-app

# Full setup wizard (recommended)
burd setup

# Or step by step:
burd analyze        # See current configuration
burd env check      # Check what needs to change
burd env fix        # Fix issues interactively
burd link           # Link if not already linked
```

### Diagnosing issues

```bash
# Check all services and current project
burd doctor

# Common fixes it suggests:
burd link           # If project not linked
burd env fix        # If .env doesn't match Burd services
burd db create <name>  # If database doesn't exist
```

### Quick database operations

```bash
# Backup before making changes
burd db export myapp -o backup.sql

# Make your changes...

# If something goes wrong, restore
burd db drop myapp --force
burd db create myapp
burd db import myapp backup.sql
```

### Sharing for client review

```bash
cd ~/projects/client-site
burd share --subdomain client-preview

# Share the URL: https://client-preview.your-domain.com
```

---

## Troubleshooting

### "No database instances configured"

Create a MariaDB or PostgreSQL instance in the Burd app first:
1. Open Burd
2. Go to Instances
3. Click "Add Instance"
4. Select MariaDB or PostgreSQL

### "Directory is not linked"

The current directory hasn't been set up with Burd:
```bash
burd link
```

### ".env file not found"

The project doesn't have a `.env` file. For Laravel:
```bash
cp .env.example .env
php artisan key:generate
```

### "frpc not running"

Start the tunnel service in Burd:
1. Open Burd
2. Go to Tunnels
3. Click "Start"
