# Burd Update System

A comprehensive guide to versioning, releases, and auto-updates for Burd.

## Overview

The update system consists of:

1. **Semantic Versioning** - Consistent version numbering
2. **Changelog Management** - Track all changes
3. **Update Server** - Hosts releases and update metadata
4. **Tauri Auto-Updater** - Built-in app updates
5. **CLI Self-Update** - `burd upgrade` command
6. **Release Pipeline** - CI/CD for building and publishing

---

## 1. Versioning Strategy

### Semantic Versioning (SemVer)

Format: `MAJOR.MINOR.PATCH` (e.g., `1.2.3`)

| Component | When to Increment | Example |
|-----------|-------------------|---------|
| **MAJOR** | Breaking changes, major rewrites | 1.0.0 → 2.0.0 |
| **MINOR** | New features, backwards compatible | 1.0.0 → 1.1.0 |
| **PATCH** | Bug fixes, small improvements | 1.0.0 → 1.0.1 |

### Pre-release Versions

- Alpha: `1.0.0-alpha.1` - Early development, unstable
- Beta: `1.0.0-beta.1` - Feature complete, testing
- RC: `1.0.0-rc.1` - Release candidate, final testing

### Version Locations

Keep versions in sync across:

```
src-tauri/Cargo.toml        → version = "1.0.0"
src-tauri/tauri.conf.json   → "version": "1.0.0"
package.json                → "version": "1.0.0"
```

### Version Bump Script

Create `scripts/bump-version.sh`:

```bash
#!/bin/bash
# Usage: ./scripts/bump-version.sh 1.2.3

VERSION=$1

if [ -z "$VERSION" ]; then
  echo "Usage: $0 <version>"
  exit 1
fi

# Update Cargo.toml
sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" src-tauri/Cargo.toml

# Update tauri.conf.json
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" src-tauri/tauri.conf.json

# Update package.json
sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" package.json

echo "Version bumped to $VERSION"
echo ""
echo "Next steps:"
echo "  1. Update CHANGELOG.md"
echo "  2. git add -A && git commit -m \"chore: bump version to $VERSION\""
echo "  3. git tag v$VERSION"
echo "  4. git push && git push --tags"
```

---

## 2. Changelog Management

### CHANGELOG.md Format

Follow [Keep a Changelog](https://keepachangelog.com/):

```markdown
# Changelog

All notable changes to Burd will be documented in this file.

## [Unreleased]

### Added
- New features go here

### Changed
- Changes to existing features

### Fixed
- Bug fixes

### Removed
- Removed features

## [1.1.0] - 2024-01-15

### Added
- `burd setup` command for full project setup wizard
- `burd doctor` command for health checks
- `burd new` command for project scaffolding

### Fixed
- Database port detection for Bedrock projects

## [1.0.0] - 2024-01-01

### Added
- Initial release
- FrankenPHP, MariaDB, PostgreSQL, Redis, Mailpit, Meilisearch support
- CLI tools: analyze, link, park, db, env, share
```

### Changelog Guidelines

1. Write for users, not developers
2. Group by type (Added, Changed, Fixed, Removed)
3. Most important changes first within each group
4. Link to issues/PRs where relevant
5. Update `[Unreleased]` section during development
6. Move to versioned section on release

---

## 3. Update Server Architecture

### Option A: Static Hosting (Recommended for Start)

Use Cloudflare R2, AWS S3, or GitHub Releases for simplicity.

```
updates.burd.dev/
├── latest.json              # Current version metadata
├── releases/
│   ├── 1.0.0/
│   │   ├── Burd_1.0.0_aarch64.dmg
│   │   ├── Burd_1.0.0_aarch64.dmg.sig
│   │   ├── Burd_1.0.0_x64.dmg
│   │   ├── Burd_1.0.0_x64.dmg.sig
│   │   ├── burd-cli-darwin-aarch64
│   │   ├── burd-cli-darwin-aarch64.sig
│   │   ├── burd-cli-darwin-x64
│   │   └── burd-cli-darwin-x64.sig
│   └── 1.1.0/
│       └── ...
└── cli/
    └── latest.json          # CLI version metadata
```

### Option B: Custom Update Server (Future)

For advanced features like:
- Staged rollouts (10% → 50% → 100%)
- Update analytics
- Channel management (stable, beta, nightly)
- Forced updates for security patches

### Update Manifest Format

**`latest.json`** (Tauri format):

```json
{
  "version": "1.1.0",
  "notes": "## What's New\n\n- Added burd setup command\n- Added burd doctor command\n- Fixed database detection",
  "pub_date": "2024-01-15T12:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRbE...",
      "url": "https://updates.burd.dev/releases/1.1.0/Burd_1.1.0_aarch64.app.tar.gz"
    },
    "darwin-x86_64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVRbE...",
      "url": "https://updates.burd.dev/releases/1.1.0/Burd_1.1.0_x64.app.tar.gz"
    }
  }
}
```

**`cli/latest.json`** (CLI format):

```json
{
  "version": "1.1.0",
  "notes": "Added burd setup, doctor, and new commands",
  "pub_date": "2024-01-15T12:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "url": "https://updates.burd.dev/releases/1.1.0/burd-cli-darwin-aarch64",
      "sha256": "abc123..."
    },
    "darwin-x86_64": {
      "url": "https://updates.burd.dev/releases/1.1.0/burd-cli-darwin-x64",
      "sha256": "def456..."
    }
  }
}
```

---

## 4. Tauri Auto-Updater Setup

### Generate Update Keys

```bash
# Generate a keypair for signing updates
npm run tauri signer generate -- -w ~/.tauri/burd.key

# This creates:
# ~/.tauri/burd.key      (PRIVATE - keep secret!)
# ~/.tauri/burd.key.pub  (PUBLIC - goes in tauri.conf.json)
```

**⚠️ IMPORTANT: Back up the private key securely. If lost, you cannot sign updates.**

### Configure tauri.conf.json

```json
{
  "tauri": {
    "updater": {
      "active": true,
      "dialog": true,
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk...",
      "endpoints": [
        "https://updates.burd.dev/latest.json"
      ]
    },
    "bundle": {
      "createUpdaterArtifacts": true
    }
  }
}
```

### Update Check Flow

1. App starts → checks `endpoints` URL
2. Compares remote version with current
3. If newer → shows update dialog (if `dialog: true`)
4. User accepts → downloads update
5. Verifies signature with `pubkey`
6. Installs and restarts

### Custom Update UI (Optional)

Instead of `dialog: true`, handle updates programmatically:

```rust
// src-tauri/src/updater.rs
use tauri::updater::UpdateResponse;

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<Option<UpdateInfo>, String> {
    match app.updater().check().await {
        Ok(update) => {
            if update.is_update_available() {
                Ok(Some(UpdateInfo {
                    version: update.latest_version().to_string(),
                    notes: update.body().map(|s| s.to_string()),
                    date: update.date().map(|d| d.to_string()),
                }))
            } else {
                Ok(None)
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    let update = app.updater().check().await.map_err(|e| e.to_string())?;

    if update.is_update_available() {
        update.download_and_install().await.map_err(|e| e.to_string())?;
        app.restart();
    }

    Ok(())
}
```

---

## 5. CLI Self-Update

### `burd upgrade` Command

```rust
// src/cli/upgrade.rs

use std::env;
use std::fs;
use std::io::{self, Write};
use std::process::Command;

const UPDATE_URL: &str = "https://updates.burd.dev/cli/latest.json";

#[derive(serde::Deserialize)]
struct UpdateManifest {
    version: String,
    notes: String,
    platforms: std::collections::HashMap<String, PlatformInfo>,
}

#[derive(serde::Deserialize)]
struct PlatformInfo {
    url: String,
    sha256: String,
}

pub fn run_upgrade() -> Result<(), String> {
    let current_version = env!("CARGO_PKG_VERSION");

    println!("Current version: {}", current_version);
    println!("Checking for updates...");

    // Fetch update manifest
    let manifest: UpdateManifest = reqwest::blocking::get(UPDATE_URL)
        .map_err(|e| format!("Failed to check for updates: {}", e))?
        .json()
        .map_err(|e| format!("Failed to parse update info: {}", e))?;

    // Compare versions
    if manifest.version == current_version {
        println!("You're already on the latest version.");
        return Ok(());
    }

    println!();
    println!("New version available: {} → {}", current_version, manifest.version);
    println!();
    println!("Release notes:");
    println!("{}", manifest.notes);
    println!();

    print!("Install update? [Y/n] ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;

    if input.trim().eq_ignore_ascii_case("n") {
        println!("Update cancelled.");
        return Ok(());
    }

    // Determine platform
    let platform = get_platform_key();
    let platform_info = manifest.platforms.get(&platform)
        .ok_or_else(|| format!("No update available for platform: {}", platform))?;

    // Download update
    println!("Downloading...");
    let bytes = reqwest::blocking::get(&platform_info.url)
        .map_err(|e| format!("Failed to download: {}", e))?
        .bytes()
        .map_err(|e| format!("Failed to read download: {}", e))?;

    // Verify checksum
    let hash = sha256::digest(&bytes[..]);
    if hash != platform_info.sha256 {
        return Err("Checksum verification failed. Download may be corrupted.".to_string());
    }

    // Replace binary
    let current_exe = env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;

    let backup_path = current_exe.with_extension("old");

    // Backup current binary
    fs::rename(&current_exe, &backup_path)
        .map_err(|e| format!("Failed to backup current binary: {}", e))?;

    // Write new binary
    fs::write(&current_exe, &bytes)
        .map_err(|e| {
            // Restore backup on failure
            let _ = fs::rename(&backup_path, &current_exe);
            format!("Failed to write new binary: {}", e)
        })?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&current_exe, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    // Remove backup
    let _ = fs::remove_file(&backup_path);

    println!();
    println!("Successfully updated to version {}!", manifest.version);

    Ok(())
}

fn get_platform_key() -> String {
    let arch = if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x64"
    } else {
        "unknown"
    };

    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    };

    format!("{}-{}", os, arch)
}
```

---

## 6. Release Pipeline (GitHub Actions)

### `.github/workflows/release.yml`

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

env:
  TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
  TAURI_KEY_PASSWORD: ${{ secrets.TAURI_KEY_PASSWORD }}

jobs:
  create-release:
    runs-on: ubuntu-latest
    outputs:
      release_id: ${{ steps.create_release.outputs.id }}
      version: ${{ steps.get_version.outputs.version }}
    steps:
      - uses: actions/checkout@v4

      - name: Get version from tag
        id: get_version
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT

      - name: Create Release
        id: create_release
        uses: actions/create-release@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ github.ref }}
          release_name: Burd ${{ steps.get_version.outputs.version }}
          draft: true
          prerelease: false

  build-app:
    needs: create-release
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: macos-latest
            target: aarch64-apple-darwin
            arch: aarch64
          - platform: macos-latest
            target: x86_64-apple-darwin
            arch: x64

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Setup Rust
        uses: dtolnay/rust-action@stable
        with:
          targets: ${{ matrix.target }}

      - name: Install dependencies
        run: npm ci

      - name: Build Tauri app
        uses: tauri-apps/tauri-action@v0
        with:
          releaseId: ${{ needs.create-release.outputs.release_id }}
          args: --target ${{ matrix.target }}
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}

  build-cli:
    needs: create-release
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: macos-latest
            target: aarch64-apple-darwin
            name: burd-cli-darwin-aarch64
          - platform: macos-latest
            target: x86_64-apple-darwin
            name: burd-cli-darwin-x64

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-action@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build CLI
        run: |
          cargo build --release --bin burd --target ${{ matrix.target }}
          cp target/${{ matrix.target }}/release/burd ${{ matrix.name }}

      - name: Generate checksum
        run: shasum -a 256 ${{ matrix.name }} > ${{ matrix.name }}.sha256

      - name: Upload to release
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: ./${{ matrix.name }}
          asset_name: ${{ matrix.name }}
          asset_content_type: application/octet-stream

  update-manifest:
    needs: [create-release, build-app, build-cli]
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Generate update manifests
        run: |
          # This script generates latest.json and cli/latest.json
          # and uploads them to your update server
          ./scripts/generate-update-manifest.sh ${{ needs.create-release.outputs.version }}

      - name: Upload to update server
        run: |
          # Upload to Cloudflare R2, S3, or your server
          aws s3 cp latest.json s3://burd-updates/latest.json
          aws s3 cp cli/latest.json s3://burd-updates/cli/latest.json
        env:
          AWS_ACCESS_KEY_ID: ${{ secrets.R2_ACCESS_KEY_ID }}
          AWS_SECRET_ACCESS_KEY: ${{ secrets.R2_SECRET_ACCESS_KEY }}
          AWS_ENDPOINT_URL: ${{ secrets.R2_ENDPOINT }}

  publish-release:
    needs: [create-release, build-app, build-cli, update-manifest]
    runs-on: ubuntu-latest

    steps:
      - name: Publish release
        uses: actions/github-script@v7
        with:
          script: |
            github.rest.repos.updateRelease({
              owner: context.repo.owner,
              repo: context.repo.repo,
              release_id: ${{ needs.create-release.outputs.release_id }},
              draft: false
            })
```

---

## 7. Update Server Setup (Scaleway Object Storage)

### Why Scaleway?

- Affordable pricing (€0.01/GB storage, €0.01/GB egress)
- S3-compatible API
- European data centers (GDPR friendly)
- Easy setup with custom domains

### Setup Steps

1. **Create Object Storage bucket**
   - Go to Scaleway Console → Object Storage
   - Create bucket: `burd-updates`
   - Region: `fr-par` (Paris), `nl-ams` (Amsterdam), or `pl-waw` (Warsaw)
   - Visibility: Public (for read access)

2. **Set up custom domain (optional)**
   - Add CNAME record: `updates.burd.dev` → `burd-updates.s3.fr-par.scw.cloud`
   - Or use the default URL: `https://burd-updates.s3.fr-par.scw.cloud`

3. **Configure bucket policy for public read**
   ```json
   {
     "Version": "2023-04-17",
     "Statement": [
       {
         "Effect": "Allow",
         "Principal": "*",
         "Action": ["s3:GetObject"],
         "Resource": ["burd-updates/*"]
       }
     ]
   }
   ```

4. **Configure CORS**
   ```xml
   <CORSConfiguration>
     <CORSRule>
       <AllowedOrigin>*</AllowedOrigin>
       <AllowedMethod>GET</AllowedMethod>
       <AllowedMethod>HEAD</AllowedMethod>
       <AllowedHeader>*</AllowedHeader>
       <MaxAgeSeconds>3600</MaxAgeSeconds>
     </CORSRule>
   </CORSConfiguration>
   ```

5. **Create API key for CI**
   - Go to Scaleway Console → IAM → API Keys
   - Create key with Object Storage permissions
   - Save `Access Key` and `Secret Key`

6. **Add secrets to GitHub**
   ```
   SCW_ACCESS_KEY_ID      # Your Scaleway access key
   SCW_SECRET_ACCESS_KEY  # Your Scaleway secret key
   SCW_REGION             # e.g., fr-par
   ```

### Scaleway Endpoints

| Region | Endpoint |
|--------|----------|
| Paris | `https://s3.fr-par.scw.cloud` |
| Amsterdam | `https://s3.nl-ams.scw.cloud` |
| Warsaw | `https://s3.pl-waw.scw.cloud` |

---

## 8. Code Signing (macOS)

### Apple Developer Requirements

1. **Apple Developer Program** ($99/year)
2. **Developer ID Application** certificate
3. **Notarization** (required for macOS 10.15+)

### Setup

1. **Export certificate**
   - Keychain Access → Export certificate as .p12
   - Base64 encode: `base64 -i certificate.p12 | pbcopy`

2. **Add to GitHub secrets**
   ```
   APPLE_CERTIFICATE: (base64 encoded .p12)
   APPLE_CERTIFICATE_PASSWORD: (password for .p12)
   APPLE_SIGNING_IDENTITY: "Developer ID Application: Your Name (TEAM_ID)"
   APPLE_ID: your@email.com
   APPLE_PASSWORD: (app-specific password)
   APPLE_TEAM_ID: XXXXXXXXXX
   ```

3. **Configure tauri.conf.json**
   ```json
   {
     "tauri": {
       "bundle": {
         "macOS": {
           "signingIdentity": "-",
           "entitlements": null
         }
       }
     }
   }
   ```

---

## 9. Release Checklist

### Before Release

- [ ] All tests passing
- [ ] Update version in all files (`./scripts/bump-version.sh X.Y.Z`)
- [ ] Update CHANGELOG.md (move Unreleased → version)
- [ ] Update any documentation
- [ ] Test on clean machine

### Release

```bash
# 1. Bump version
./scripts/bump-version.sh 1.2.0

# 2. Commit
git add -A
git commit -m "chore: release v1.2.0"

# 3. Tag
git tag v1.2.0

# 4. Push (triggers CI)
git push && git push --tags
```

### After Release

- [ ] Verify GitHub Release is published
- [ ] Verify update manifest is live
- [ ] Test auto-update from previous version
- [ ] Test CLI upgrade
- [ ] Announce release (Discord, Twitter, etc.)

---

## 10. Update Channels (Future)

For different release tracks:

```
updates.burd.dev/
├── stable/
│   └── latest.json
├── beta/
│   └── latest.json
└── nightly/
    └── latest.json
```

Configure in app settings which channel to follow.

---

## 11. Rollback Strategy

If a release has critical bugs:

1. **Immediate**: Update `latest.json` to point to previous version
2. **Quick fix**: Release patch version ASAP
3. **Communication**: In-app banner or notification

### Emergency Rollback Script

```bash
#!/bin/bash
# scripts/rollback.sh

PREVIOUS_VERSION=$1

if [ -z "$PREVIOUS_VERSION" ]; then
  echo "Usage: $0 <previous-version>"
  exit 1
fi

# Update manifests to point to previous version
aws s3 cp s3://burd-updates/releases/$PREVIOUS_VERSION/latest.json s3://burd-updates/latest.json

echo "Rolled back to version $PREVIOUS_VERSION"
```

---

## Quick Start

### Minimum Viable Update System

1. **Generate signing key**
   ```bash
   npm run tauri signer generate -- -w ~/.tauri/burd.key
   ```

2. **Configure tauri.conf.json**
   ```json
   {
     "tauri": {
       "updater": {
         "active": true,
         "dialog": true,
         "pubkey": "<your-public-key>",
         "endpoints": ["https://updates.burd.dev/latest.json"]
       }
     }
   }
   ```

3. **Set up R2 bucket** at `updates.burd.dev`

4. **Add GitHub secrets** for CI

5. **Create first release**
   ```bash
   ./scripts/bump-version.sh 1.0.0
   git add -A && git commit -m "chore: release v1.0.0"
   git tag v1.0.0
   git push && git push --tags
   ```

The CI will build, sign, and publish automatically.
