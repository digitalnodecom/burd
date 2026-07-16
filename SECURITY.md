# Security Policy

## Supported Versions

Only the latest [released version](https://github.com/digitalnodecom/burd/releases) of Burd is supported. Security fixes ship in new releases; older versions do not receive backports. Please update before reporting an issue.

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public issues, discussions, or pull requests.**

Report privately through GitHub Security Advisories:

1. Go to the [Security tab](https://github.com/digitalnodecom/burd/security) of this repository.
2. Click **Report a vulnerability**.
3. Describe the issue with enough detail to reproduce it.

Helpful details include:

- Affected version and macOS version/architecture
- Component involved (desktop app, `burd` CLI, privileged helper, local API, Caddy/SSL handling)
- Steps to reproduce, and the impact you believe it has
- Any proof-of-concept, logs, or configuration needed to trigger it

## What to Expect

Burd is maintained by a small team, so responses are best-effort:

- We aim to acknowledge new reports within about a week.
- We will keep you updated as we investigate and work toward a fix.
- Once a fix is released we will publish an advisory and credit you, unless you prefer to remain anonymous.

Please give us a reasonable opportunity to release a fix before disclosing the issue publicly.

## Scope

Burd is a local-first development tool for macOS. It runs on a developer's own machine and is not intended to be exposed to untrusted networks.

The most sensitive parts of Burd, and the areas where we are most interested in reports, are:

- The **privileged helper daemon**, which runs with elevated permissions
- The **local HTTP API** and MCP server on localhost, and anything that can reach them
- The **local Caddy certificate authority** and the trust it is granted in the system keychain
- **Service binary downloads**, updates, and the CLI self-update path

Out of scope:

- Vulnerabilities in the third-party services Burd downloads and manages (PHP/FrankenPHP, MariaDB, MySQL, PostgreSQL, MongoDB, Redis, Valkey, Caddy, and others) — please report those to their respective projects.
- Issues that require an attacker to already have local administrator access, or physical access to an unlocked machine.
- Development-oriented defaults that are intentional for a local environment (for example, default service credentials on localhost-bound instances), unless you can show impact beyond the local machine.

If you are unsure whether something is in scope, report it anyway — we would rather look at it.
