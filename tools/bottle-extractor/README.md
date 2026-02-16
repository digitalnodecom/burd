# Homebrew Bottle Extractor

A toolkit to extract pre-compiled binaries from Homebrew bottles and repackage them as self-contained, portable packages.

## Why?

Homebrew bottles contain pre-compiled binaries, but they:
- Require Homebrew to be installed
- Link to dylibs at Homebrew-specific paths (`/opt/homebrew/opt/...`)
- Can't be easily redistributed

This toolkit extracts binaries and relinks them to use `@executable_path`, making them fully portable (like Laravel Herd does).

## Quick Start

```bash
# Extract Redis for arm64 macOS Sonoma
./extract.sh redis

# Extract specific version
./extract.sh redis 8.4.0 arm64 sonoma

# Extract for Intel Mac
./extract.sh redis 8.4.0 x86_64 sonoma
```

## Output Structure

```
output/redis/8.4.0-arm64/
├── bin/
│   ├── redis-server
│   ├── redis-cli
│   └── ...
├── lib/
│   ├── libssl.3.dylib
│   └── libcrypto.3.dylib
├── etc/
│   ├── redis.conf
│   └── redis-sentinel.conf
└── manifest.json
```

## How It Works

1. **Fetch**: Downloads Homebrew bottle using `brew fetch`
2. **Extract**: Unpacks the .tar.gz and organizes files
3. **Bundle**: Fetches dependency bottles (e.g., OpenSSL) and copies required dylibs
4. **Relink**: Uses `install_name_tool` to change paths from `@@HOMEBREW_PREFIX@@/...` to `@executable_path/../lib/...`
5. **Verify**: Tests that binaries run without Homebrew

## Comparison: Before vs After

**Before (Homebrew bottle):**
```
$ otool -L redis-server
  @@HOMEBREW_PREFIX@@/opt/openssl@3/lib/libssl.3.dylib
  @@HOMEBREW_PREFIX@@/opt/openssl@3/lib/libcrypto.3.dylib
```

**After (extracted):**
```
$ otool -L redis-server
  @executable_path/../lib/libssl.3.dylib
  @executable_path/../lib/libcrypto.3.dylib
```

## Requirements

- macOS (tested on Sonoma/Sequoia)
- Homebrew (for fetching bottles)
- Standard macOS tools: `otool`, `install_name_tool`

## Formula Configs

Each supported formula has a config in `formulas/`:

```json
{
  "name": "redis",
  "dependencies": [
    {"formula": "openssl@3", "libs": ["libssl.3.dylib", "libcrypto.3.dylib"]}
  ],
  "binaries": ["redis-server", "redis-cli", ...],
  "test_commands": ["{main_binary} --version"]
}
```

## Adding New Formulas

1. Create `formulas/<name>.json` with dependency info
2. Run `./extract.sh <name>`
3. Test the output binaries
4. Upload to your CDN

---

## PostgreSQL Special Handling

PostgreSQL requires special handling due to hardcoded paths compiled into the binaries. This section documents the problem, solution, and limitations.

### The Problem

PostgreSQL binaries have hardcoded paths compiled in at build time:

```
SHAREDIR = /opt/homebrew/share/postgresql@17  (33 chars)
PKGLIBDIR = /opt/homebrew/lib/postgresql@17   (31 chars)
```

These paths are used for:
- **Timezone data** (`share/timezonesets/Default`, `share/timezone/`)
- **Extensions** (`lib/*.so`, `lib/*.dylib`)
- **Locale files, SQL templates, system catalogs** (`share/*.sql`)

When PostgreSQL can't find these files, you get errors like:
```
FATAL: invalid value for parameter "timezone_abbreviations": "Default"
FATAL: invalid value for parameter "TimeZone": "Europe/Madrid"
```

### Why Standard Solutions Don't Work

**Environment Variables:**
- `PGSHAREDIR` and `PKGLIBDIR` are **NOT** standard PostgreSQL environment variables
- PostgreSQL does not honor them at runtime
- The hardcoded paths are in `pg_config_paths.h`, compiled into the binaries
- You can verify with: `pg_config --sharedir` → shows hardcoded path

**The `-L` flag for initdb:**
- Only affects initdb's own share file lookups
- initdb spawns a `postgres` subprocess for verification
- That subprocess uses the hardcoded paths from the binary, ignoring `-L`

**Using user home paths:**
- Paths like `~/Library/Application Support/Burd/bin/postgresql/17.7/share` are 60+ characters
- The original hardcoded path is only 33 characters
- Binary patching cannot make the binary larger (see below)

### The Solution: Binary Patching + Symlinks

**Step 1: Binary Patching**

Replace hardcoded paths in the binary with shorter paths, padded with null bytes:

```
Before: /opt/homebrew/share/postgresql@17  (33 bytes)
After:  /opt/burd/share\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0  (33 bytes)
```

Python example:
```python
with open('postgres', 'rb') as f:
    data = f.read()

old_path = b'/opt/homebrew/share/postgresql@17'  # 33 bytes
new_path = b'/opt/burd/share' + b'\x00' * 18     # 15 + 18 = 33 bytes

data = data.replace(old_path, new_path)

with open('postgres', 'wb') as f:
    f.write(data)
```

**Step 2: Re-sign (Apple Silicon)**

After modifying the binary, it must be re-signed:
```bash
codesign --remove-signature postgres
codesign -s - postgres
```

**Step 3: Create Symlinks**

Create symlinks at the short path pointing to actual files:
```bash
ln -sfn "/path/to/actual/share" /opt/burd/share
ln -sfn "/path/to/actual/lib" /opt/burd/lib
```

### Critical: Path Length Limitation

**The Rule:** New path must be **SHORTER OR EQUAL** to the original path length.

| Path Type | Original Path | Length | New Path | Length |
|-----------|---------------|--------|----------|--------|
| SHAREDIR | `/opt/homebrew/share/postgresql@17` | 33 | `/opt/burd/share` | 15 |
| PKGLIBDIR | `/opt/homebrew/lib/postgresql@17` | 31 | `/opt/burd/lib` | 13 |

**Why this limitation exists:**
- Binary patching replaces bytes **in-place**
- You cannot insert additional bytes (would corrupt the binary)
- You cannot make the binary larger
- Excess space must be filled with null bytes (`\0`)
- Null bytes act as string terminators in C

**What this means:**
- User home paths (`~/Library/...`) are too long (60+ chars)
- We need a short, fixed path like `/opt/burd/`
- This path must exist and be writable by the user

### Implementation in Burd

**Files involved:**
- `src-tauri/src/binary.rs` → `patch_postgresql_paths()` - does the patching
- `src-tauri/src/binary.rs` → `copy_bundled_package()` - calls patching for PostgreSQL
- `src-tauri/helper/main.rs` → `SetupOptBurd` - creates `/opt/burd` with user ownership

**Flow when downloading PostgreSQL:**
1. Archive downloaded and extracted to `~/Library/Application Support/Burd/bin/postgresql/17.7/`
2. Binaries in `bin/` directory scanned for hardcoded paths
3. Each binary patched: `/opt/homebrew/share/postgresql@17` → `/opt/burd/share`
4. Each binary re-signed with ad-hoc signature
5. Symlinks created in `/opt/burd/` pointing to actual directories
6. PostgreSQL can now initialize and run correctly

### Setup Requirements

**One-time setup (requires sudo):**
```bash
sudo mkdir -p /opt/burd
sudo chown $USER:staff /opt/burd
```

In Burd, this is handled by the privileged helper tool on first run.

**Apple Silicon requirements:**
- All executables must be signed to run
- `codesign --force --sign -` creates an ad-hoc signature
- Must re-sign after **any** binary modification
- Unsigned/invalid binaries will fail with "killed" or code signing errors

### Debugging Tips

**Check what paths are hardcoded:**
```bash
strings postgres | grep homebrew
pg_config --sharedir
pg_config --pkglibdir
```

**Verify patching worked:**
```bash
strings postgres | grep "/opt/burd"
```

**Check signature:**
```bash
codesign -dv postgres
codesign -v postgres  # verify
```

**Test initialization:**
```bash
export TZ=GMT
./initdb -D /tmp/pgdata --auth=trust --no-locale --encoding=UTF8
```

## License

MIT
