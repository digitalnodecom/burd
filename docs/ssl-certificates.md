# SSL Certificates - Reverse Proxy Setup

## Overview

Burd uses Caddy as the reverse proxy with automatic internal SSL certificate generation via `tls internal`. Each HTTPS-enabled domain gets its own certificate signed by a local Caddy Root CA.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Caddy Daemon (root)                       │
│  - Runs via launchd as com.burd.proxy                       │
│  - Listens on ports 80 (HTTP) and 443 (HTTPS)               │
│  - Uses XDG_DATA_HOME for certificate storage               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│              Certificate Storage (Daemon)                    │
│  /Library/Application Support/Burd/caddy-data/caddy/pki/    │
│    └── authorities/local/                                    │
│        ├── root.crt      ← Root CA (needs to be trusted)    │
│        ├── root.key      ← Root CA private key              │
│        ├── intermediate.crt                                  │
│        └── intermediate.key                                  │
└─────────────────────────────────────────────────────────────┘
```

## Certificate Locations

### Daemon Path (Primary - when proxy is installed)
```
/Library/Application Support/Burd/caddy-data/caddy/pki/authorities/local/root.crt
```
- Used when Caddy runs as a launchd daemon
- Directory is owned by root (restricted permissions)
- The helper tool (runs as root) reads this path

### User Path (Fallback - when running Caddy manually)
```
~/Library/Application Support/Caddy/pki/authorities/local/root.crt
```
- Used when Caddy runs as a regular user (not via launchd)
- Only checked if proxy daemon is not installed

## How SSL Works

1. **Domain Configuration**: When SSL is enabled on a domain, the Caddyfile entry uses:
   ```
   https://example.test {
       tls internal
       reverse_proxy localhost:8000
   }
   ```

2. **Certificate Generation**: On first HTTPS request to a domain, Caddy:
   - Generates a Root CA (if not exists)
   - Generates an Intermediate CA
   - Issues a domain-specific certificate

3. **Certificate Naming**: The Root CA is named based on the year, e.g.:
   - `Caddy Local Authority - 2026 ECC Root`

## Trusting the CA Certificate

### Why Trust is Needed
Browsers will show security warnings until the Root CA is trusted in the system keychain.

### Manual Trust Process (Recommended)
1. Open **Keychain Access** (via UI button or `/System/Applications/Utilities/Keychain Access.app`)
2. Find the certificate in **System** keychain (e.g., "Caddy Local Authority - 2026 ECC Root")
3. Double-click the certificate
4. Expand the **Trust** section
5. Set **When using this certificate** to **Always Trust**
6. Close and enter admin password when prompted

### Programmatic Trust (Complex)
Due to macOS security restrictions, programmatic trust requires:
- `security authorizationdb write com.apple.trust-settings.admin allow` (temporarily allow)
- `security add-trusted-cert -d -r trustRoot -p ssl -k /Library/Keychains/System.keychain <cert>`
- `security authorizationdb remove com.apple.trust-settings.admin` (remove permission)

This is implemented but can be unreliable due to GUI authorization requirements.

## Helper Tool Integration

The privileged helper (`burd-helper`) handles certificate operations:

### GetCertInfo Request
Reads certificate metadata (since daemon path is root-owned):
```rust
HelperRequest::GetCertInfo { cert_path: String }
// Returns: "exists|<cert_name>|<expiry>" or "not_found"
```

### IsCaddyCATrusted Request
Verifies if the certificate is trusted for SSL:
```rust
HelperRequest::IsCaddyCATrusted { cert_path: String }
// Uses: security verify-cert -c <path> -p ssl -l
// Returns: "trusted" or "not_trusted"
```

## Caddyfile Structure

Global settings in `/Library/Application Support/Burd/Caddyfile`:
```
{
    admin off
    local_certs
}

import domains/*.caddy

http://*.test {
    respond "No service configured for this domain" 404
}
```

Per-domain configs in `/Library/Application Support/Burd/domains/<domain>.caddy`:
```
# HTTP only
http://example.test {
    reverse_proxy localhost:8000
}

# With SSL
https://example.test {
    tls internal
    reverse_proxy localhost:8000
}
```

## Launchd Configuration

The proxy daemon plist (`/Library/LaunchDaemons/com.burd.proxy.plist`) sets:
```xml
<key>EnvironmentVariables</key>
<dict>
    <key>XDG_DATA_HOME</key>
    <string>/Library/Application Support/Burd/caddy-data</string>
</dict>
```

This ensures Caddy stores its PKI data in a system location rather than user home.

## Troubleshooting

### Certificate shows "Not Trusted"
1. Ensure the helper is installed and running
2. Check if the correct certificate is in System keychain
3. Manually set trust via Keychain Access

### Multiple Certificates in Keychain
Old certificates may remain from previous installs. Remove duplicates:
```bash
security delete-certificate -c "Caddy Local Authority" /Library/Keychains/System.keychain
```

### Certificate Not Found
1. Verify an SSL domain exists and has been accessed via HTTPS
2. Check daemon logs: `/Library/Logs/Burd/caddy.log`
3. Verify the caddy-data path exists and has correct permissions

### Checking Certificate Details
```bash
# View cert in daemon path (requires sudo)
sudo openssl x509 -in "/Library/Application Support/Burd/caddy-data/caddy/pki/authorities/local/root.crt" -noout -subject -dates

# Verify trust status
security verify-cert -c <path> -p ssl -l
```

## Code References

- Certificate path logic: `src-tauri/src/commands/proxy.rs` → `get_caddy_ca_path()`
- Trust status check: `src-tauri/src/commands/proxy.rs` → `get_ca_trust_status_internal()`
- Helper cert operations: `src-tauri/helper/main.rs` → `get_cert_info()`, `is_caddy_ca_trusted()`
- UI component: `src/lib/sections/GeneralSection.svelte` → HTTPS Certificate section
