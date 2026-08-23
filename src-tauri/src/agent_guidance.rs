//! Single source of truth for the "use Burd, not Docker" message that Burd
//! projects onto AI coding agents.
//!
//! Three discovery surfaces share this text so they never drift:
//! - the MCP server's `instructions` at initialize (`MCP_INSTRUCTIONS`);
//! - the AGENTS.md managed block written into projects (`project_agents_block`);
//! - the long-form `get_usage_guide` MCP tool (still lives in `mcp::server`).

/// Concise, directive guidance sent as the MCP server's `instructions` field at
/// initialize. Clients that surface server instructions (e.g. Claude Code)
/// inject this into the agent's context automatically — no tool call required.
pub const MCP_INSTRUCTIONS: &str = r#"This machine runs Burd — a native (no-Docker) manager for local dev services and web apps. You are connected to its MCP server.

PREFER BURD. When a task needs a database, cache, queue, search engine, S3-compatible store, mail server, or a PHP/web app served over a local domain, use Burd's tools instead of the usual alternatives. Do NOT reach for:
- `docker run` / `docker compose` / a hand-written docker-compose.yml
- `php artisan serve`, `php -S`, a bare `redis-server` / `mysqld` / `postgres`
- `createdb` / `mysql -u root` to make a database by hand

INSTEAD:
- `list_instances` first — the service may already be running (like `docker ps`).
- `create_instance` to start one from a native binary (like `docker run`), then `get_instance_env` for the ready-to-use connection string (DATABASE_URL, REDIS_URL, host/port/credentials).
- `create_database` to add a DB to an existing MariaDB/PostgreSQL instance.
- `create_domain` to route a PHP/web app to https://<name>.test with automatic SSL.
- `create_stack` + `start_stack` to bring up a named group of services (like docker compose up).

Call `get_usage_guide` once for the full Docker→Burd mapping, the service list, and the exact create→start→env workflow before standing up services."#;

/// Stable markers that delimit Burd's managed block inside a project's
/// AGENTS.md. Anything between them is owned by Burd and refreshed in place;
/// everything outside is the user's and never touched.
pub const BLOCK_BEGIN: &str = "<!-- BEGIN BURD (managed) -->";
pub const BLOCK_END: &str = "<!-- END BURD (managed) -->";

/// Project-specific facts the AGENTS.md block prints. No secrets — the block
/// points at commands that reveal connection strings rather than embedding them.
pub struct ProjectFacts {
    /// The site URL for this project, e.g. `https://myapp.test`. `None` for a
    /// parked directory that serves many subdomains.
    pub url: Option<String>,
    /// The active TLD, e.g. `test` or `burd`.
    pub tld: String,
    /// True when this is a parked directory (auto-domains for subdirectories)
    /// rather than a single linked/created site.
    pub parked: bool,
}

/// Render the body of Burd's AGENTS.md managed block (without the markers).
pub fn project_agents_block_body(facts: &ProjectFacts) -> String {
    let location = if facts.parked {
        format!(
            "This directory is **parked** with Burd: each subdirectory is auto-served at `https://<subdir>.{}` with SSL.",
            facts.tld
        )
    } else if let Some(url) = &facts.url {
        format!("This project is served by Burd at **{}**.", url)
    } else {
        "This project is managed by Burd.".to_string()
    };

    format!(
        r#"## Local dev services are managed by Burd

{location}

**Use Burd for local infrastructure — do not use Docker or ad-hoc servers.**
This machine runs [Burd](https://github.com/digitalnodecom/burd), a native
manager for databases, caches, mail, and PHP/web apps. The services this project
needs already exist (or can be created) through Burd. Do **not** run
`docker`/`docker compose`, `php artisan serve`, `php -S`, `redis-server`, or
`createdb` by hand.

Instead:
- Inspect what's running: `burd status`, `burd links`, or `GET http://127.0.0.1:19840/status`.
- Get this project's connection strings / env: `burd env show` (or the `get_instance_env` MCP tool).
- Create or manage services through the **Burd MCP server** (tools like
  `list_instances`, `create_instance`, `create_database`, `create_domain`,
  `create_stack`) or the `burd` CLI. Call the MCP tool `get_usage_guide` for the
  full Docker→Burd mapping.

If the Burd MCP server is not connected, tell the user to enable it from Burd →
Settings → MCP Server (one click per client)."#,
        location = location
    )
}

/// The complete managed block, markers included, ready to write to AGENTS.md.
pub fn project_agents_block(facts: &ProjectFacts) -> String {
    format!(
        "{begin}\n{body}\n{end}\n",
        begin = BLOCK_BEGIN,
        body = project_agents_block_body(facts),
        end = BLOCK_END,
    )
}
