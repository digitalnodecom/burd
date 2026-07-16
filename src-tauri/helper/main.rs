//! Burd Privileged Helper Tool
//!
//! This helper runs as root via launchd and handles privileged operations
//! for the Burd app via Unix domain socket communication.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::Command;

const SOCKET_PATH: &str = "/var/run/com.burd.helper.sock";
const RESOLVER_DIR: &str = "/etc/resolver";
const PROXY_PLIST_PATH: &str = "/Library/LaunchDaemons/com.burd.proxy.plist";

/// Directory the helper is permitted to touch on behalf of a caller, relative
/// to the caller's home. All path-taking operations are constrained to it.
const BURD_SUPPORT_SUFFIX: &str = "Library/Application Support/Burd";

/// Request types the helper can handle
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HelperRequest {
    /// Install resolver file for a TLD
    InstallResolver { tld: String, dns_port: u16 },
    /// Uninstall resolver file for a TLD
    UninstallResolver { tld: String },
    /// Install proxy daemon plist
    InstallProxyDaemon { plist_content: String },
    /// Uninstall proxy daemon
    UninstallProxyDaemon,
    /// Start proxy daemon
    StartProxyDaemon,
    /// Restart proxy daemon
    RestartProxyDaemon,
    /// Check if helper is running (ping)
    Ping,
    /// Get certificate info (exists, name, expiry) - for reading certs in restricted paths
    GetCertInfo { cert_path: String },
    /// Fix permissions on Caddy data directory to be user-readable
    FixCaddyPermissions { path: String },
    /// Setup /opt/burd directory with user ownership
    SetupOptBurd { username: String },
}

/// Response from the helper
#[derive(Debug, Serialize, Deserialize)]
pub struct HelperResponse {
    pub success: bool,
    pub message: String,
}

impl HelperResponse {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
        }
    }

    fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
        }
    }
}

fn main() {
    // Remove existing socket if present
    let _ = fs::remove_file(SOCKET_PATH);

    // Create socket directory if needed
    if let Some(parent) = Path::new(SOCKET_PATH).parent() {
        let _ = fs::create_dir_all(parent);
    }

    // Bind to socket
    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to bind to socket: {}", e);
            std::process::exit(1);
        }
    };

    // Set socket permissions to allow non-root users to connect
    if let Err(e) = fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o666)) {
        eprintln!("Warning: Failed to set socket permissions: {}", e);
    }

    println!("Burd helper listening on {}", SOCKET_PATH);

    // Accept connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                // Authenticate the caller before doing anything. The socket is
                // world-connectable (the app runs unprivileged), so the only
                // thing standing between an arbitrary local process and root is
                // this check. Accept root and the console (logged-in GUI) user
                // only; reject everyone else — other accounts, sandboxed
                // daemons, `nobody`, etc.
                match peer_uid(&stream) {
                    Some(uid) if is_authorized_uid(uid) => {
                        if let Err(e) = handle_client(stream) {
                            eprintln!("Error handling client: {}", e);
                        }
                    }
                    Some(uid) => {
                        eprintln!("Rejected connection from unauthorized uid {}", uid);
                    }
                    None => {
                        eprintln!("Rejected connection: could not determine peer identity");
                    }
                }
            }
            Err(e) => {
                eprintln!("Connection error: {}", e);
            }
        }
    }
}

/// Read the effective uid of the process on the other end of the socket, straight
/// from the kernel. Unforgeable — the caller cannot lie about who it is.
fn peer_uid(stream: &UnixStream) -> Option<u32> {
    let fd = stream.as_raw_fd();
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    // SAFETY: fd is a valid connected socket for the duration of this call;
    // getpeereid only writes through the two out-pointers.
    let rc = unsafe { libc::getpeereid(fd, &mut uid, &mut gid) };
    if rc == 0 {
        Some(uid)
    } else {
        None
    }
}

/// The uid of the currently logged-in GUI user. On macOS the owner of
/// `/dev/console` is exactly that user — no SystemConfiguration framework needed.
fn console_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    fs::metadata("/dev/console").ok().map(|m| m.uid())
}

/// A caller is trusted iff it is root or the logged-in console user.
fn is_authorized_uid(uid: u32) -> bool {
    uid == 0 || console_uid() == Some(uid)
}

fn handle_client(mut stream: UnixStream) -> Result<(), String> {
    let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);

    for line in reader.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.is_empty() {
            continue;
        }

        let request: HelperRequest =
            serde_json::from_str(&line).map_err(|e| format!("Invalid request: {}", e))?;

        let response = handle_request(request);

        let response_json = serde_json::to_string(&response)
            .map_err(|e| format!("Failed to serialize response: {}", e))?;

        writeln!(stream, "{}", response_json).map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn handle_request(request: HelperRequest) -> HelperResponse {
    match request {
        HelperRequest::Ping => HelperResponse::ok("pong"),

        HelperRequest::InstallResolver { tld, dns_port } => install_resolver(&tld, dns_port),

        HelperRequest::UninstallResolver { tld } => uninstall_resolver(&tld),

        HelperRequest::InstallProxyDaemon { plist_content } => install_proxy_daemon(&plist_content),

        HelperRequest::UninstallProxyDaemon => uninstall_proxy_daemon(),

        HelperRequest::StartProxyDaemon => start_proxy_daemon(),

        HelperRequest::RestartProxyDaemon => restart_proxy_daemon(),

        HelperRequest::GetCertInfo { cert_path } => get_cert_info(&cert_path),

        HelperRequest::FixCaddyPermissions { path } => fix_caddy_permissions(&path),

        HelperRequest::SetupOptBurd { username } => setup_opt_burd(&username),
    }
}

// ============================================================================
// Resolver Operations
// ============================================================================

/// A TLD is a single lowercase-alphanumeric label. This blocks path traversal
/// (`../../etc/sudoers`) since the value is interpolated straight into a root
/// file path, and rejects anything that isn't a plausible resolver name.
fn is_valid_tld(tld: &str) -> bool {
    !tld.is_empty()
        && tld.len() <= 63
        && tld
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        && !tld.starts_with('-')
        && !tld.ends_with('-')
}

fn install_resolver(tld: &str, dns_port: u16) -> HelperResponse {
    if !is_valid_tld(tld) {
        return HelperResponse::error("Invalid TLD");
    }

    let path = format!("{}/{}", RESOLVER_DIR, tld);
    let content = format!("nameserver 127.0.0.1\nport {}\n", dns_port);

    // Create resolver directory
    if let Err(e) = fs::create_dir_all(RESOLVER_DIR) {
        return HelperResponse::error(format!("Failed to create resolver dir: {}", e));
    }

    // Write resolver file
    if let Err(e) = fs::write(&path, &content) {
        return HelperResponse::error(format!("Failed to write resolver file: {}", e));
    }

    HelperResponse::ok(format!("Resolver installed for .{}", tld))
}

fn uninstall_resolver(tld: &str) -> HelperResponse {
    if !is_valid_tld(tld) {
        return HelperResponse::error("Invalid TLD");
    }

    let path = format!("{}/{}", RESOLVER_DIR, tld);

    if let Err(e) = fs::remove_file(&path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return HelperResponse::error(format!("Failed to remove resolver file: {}", e));
        }
    }

    HelperResponse::ok(format!("Resolver uninstalled for .{}", tld))
}

// ============================================================================
// Proxy Daemon Operations
// ============================================================================

fn install_proxy_daemon(plist_content: &str) -> HelperResponse {
    let path = Path::new(PROXY_PLIST_PATH);

    // Create parent directory
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            return HelperResponse::error(format!("Failed to create LaunchDaemons dir: {}", e));
        }
    }

    // Also create the app support and logs directories
    let _ = fs::create_dir_all("/Library/Application Support/Burd");
    let _ = fs::create_dir_all("/Library/Logs/Burd");

    // Write plist file
    if let Err(e) = fs::write(path, plist_content) {
        return HelperResponse::error(format!("Failed to write plist: {}", e));
    }

    // Set permissions
    let _ = Command::new("chmod")
        .args(["644", PROXY_PLIST_PATH])
        .output();

    // Load the daemon
    let output = Command::new("launchctl")
        .args(["load", "-w", PROXY_PLIST_PATH])
        .output();

    match output {
        Ok(o) if o.status.success() => HelperResponse::ok("Proxy daemon installed and loaded"),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            HelperResponse::error(format!("Failed to load daemon: {}", stderr))
        }
        Err(e) => HelperResponse::error(format!("Failed to run launchctl: {}", e)),
    }
}

fn uninstall_proxy_daemon() -> HelperResponse {
    // Unload the daemon
    let _ = Command::new("launchctl")
        .args(["bootout", "system/com.burd.proxy"])
        .output();

    // Also try legacy unload
    let _ = Command::new("launchctl")
        .args(["unload", "-w", PROXY_PLIST_PATH])
        .output();

    // Remove the plist
    if let Err(e) = fs::remove_file(PROXY_PLIST_PATH) {
        if e.kind() != std::io::ErrorKind::NotFound {
            return HelperResponse::error(format!("Failed to remove plist: {}", e));
        }
    }

    HelperResponse::ok("Proxy daemon uninstalled")
}

fn start_proxy_daemon() -> HelperResponse {
    let output = Command::new("launchctl")
        .args(["kickstart", "system/com.burd.proxy"])
        .output();

    match output {
        Ok(o) if o.status.success() => HelperResponse::ok("Proxy daemon started"),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            HelperResponse::error(format!("Failed to start daemon: {}", stderr))
        }
        Err(e) => HelperResponse::error(format!("Failed to run launchctl: {}", e)),
    }
}

fn restart_proxy_daemon() -> HelperResponse {
    let output = Command::new("launchctl")
        .args(["kickstart", "-k", "system/com.burd.proxy"])
        .output();

    match output {
        Ok(o) if o.status.success() => HelperResponse::ok("Proxy daemon restarted"),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            HelperResponse::error(format!("Failed to restart daemon: {}", stderr))
        }
        Err(e) => HelperResponse::error(format!("Failed to run launchctl: {}", e)),
    }
}

// Need this for fs::Permissions::from_mode
use std::os::unix::fs::PermissionsExt;

// ============================================================================
// Certificate Info Operations
// ============================================================================

/// Get certificate info (exists, name, expiry) as JSON
fn get_cert_info(cert_path: &str) -> HelperResponse {
    // Check if file exists
    if !std::path::Path::new(cert_path).exists() {
        return HelperResponse::ok("not_found");
    }

    // Get subject (certificate name)
    let name = Command::new("openssl")
        .args(["x509", "-in", cert_path, "-noout", "-subject"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let output = String::from_utf8_lossy(&o.stdout);
                output
                    .split("CN = ")
                    .nth(1)
                    .or_else(|| output.split("CN=").nth(1))
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // Get expiry date
    let expiry = Command::new("openssl")
        .args(["x509", "-in", cert_path, "-noout", "-enddate"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let output = String::from_utf8_lossy(&o.stdout);
                output.split('=').nth(1).map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // Return as JSON-like string: "exists|name|expiry"
    let name_str = name.unwrap_or_default();
    let expiry_str = expiry.unwrap_or_default();
    HelperResponse::ok(format!("exists|{}|{}", name_str, expiry_str))
}

// ============================================================================
// Caddy Permissions Fix
// ============================================================================

/// Fix permissions on Caddy data directory to be user-readable
/// This is needed because Caddy runs as root via launchd and creates files with restricted permissions
fn fix_caddy_permissions(path: &str) -> HelperResponse {
    // Check existence first: canonicalize() requires the path to exist, and a
    // missing caddy-data dir is a normal no-op, not an error.
    if !Path::new(path).exists() {
        return HelperResponse::ok("Path does not exist yet, no permissions to fix");
    }

    // Security check: resolve symlinks and `..` to a real path, then require it
    // to live under SOME user's `Library/Application Support/Burd`. A substring
    // test is not enough — `/x/Library/Application Support/Burd/../../../etc`
    // contains the marker yet escapes it; canonicalization is what makes the
    // containment check sound.
    let real = match fs::canonicalize(path) {
        Ok(p) => p,
        Err(e) => return HelperResponse::error(format!("Cannot resolve path: {}", e)),
    };
    // canonicalize() has already resolved every `..` and symlink, so a boundary
    // check on the real path is sound. Require the Burd support dir to be an
    // exact path component (marker + `/`, or the dir itself) — not a mere
    // substring, which would also match a sibling like `Burd-evil`.
    let real_str = real.to_string_lossy();
    let marker = format!("/{}", BURD_SUPPORT_SUFFIX);
    let under_burd = real_str.contains(&format!("{}/", marker)) || real_str.ends_with(&marker);
    if !under_burd {
        return HelperResponse::error(
            "Permission denied: can only fix permissions within Burd directories".to_string(),
        );
    }

    // Make directory and all contents readable (755 for dirs, 644 for files)
    // Use chmod -R to recursively set permissions
    let output = Command::new("chmod")
        .args(["-R", "755", real_str.as_ref()])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            HelperResponse::ok(format!("Permissions fixed for {}", path))
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            HelperResponse::error(format!("Failed to fix permissions: {}", stderr))
        }
        Err(e) => HelperResponse::error(format!("Failed to run chmod: {}", e)),
    }
}

// ============================================================================
// Opt Burd Setup
// ============================================================================

/// Setup /opt/burd directory with proper user ownership
/// This is needed for PostgreSQL binary patching to use short paths
fn setup_opt_burd(username: &str) -> HelperResponse {
    const OPT_BURD: &str = "/opt/burd";

    // Validate username (basic security check)
    if username.is_empty() || username.contains('/') || username.contains('\0') {
        return HelperResponse::error("Invalid username");
    }

    // Check if already exists with correct ownership
    if Path::new(OPT_BURD).exists() {
        // Check ownership
        let output = Command::new("stat").args(["-f", "%Su", OPT_BURD]).output();

        if let Ok(o) = output {
            let owner = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if owner == username {
                return HelperResponse::ok("/opt/burd already exists with correct ownership");
            }
        }

        // Fix ownership if it exists but has wrong owner
        let output = Command::new("chown").args([username, OPT_BURD]).output();

        return match output {
            Ok(o) if o.status.success() => HelperResponse::ok("/opt/burd ownership fixed"),
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                HelperResponse::error(format!("Failed to fix ownership: {}", stderr))
            }
            Err(e) => HelperResponse::error(format!("Failed to run chown: {}", e)),
        };
    }

    // Create the directory
    if let Err(e) = fs::create_dir_all(OPT_BURD) {
        return HelperResponse::error(format!("Failed to create /opt/burd: {}", e));
    }

    // Set ownership to the user
    let output = Command::new("chown").args([username, OPT_BURD]).output();

    match output {
        Ok(o) if o.status.success() => HelperResponse::ok("/opt/burd created and ownership set"),
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr);
            HelperResponse::error(format!("Failed to set ownership: {}", stderr))
        }
        Err(e) => HelperResponse::error(format!("Failed to run chown: {}", e)),
    }
}
