<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // Open Keychain Access app
  async function openKeychain() {
    try {
      await invoke("open_keychain_access");
    } catch (e) {
      console.error("Failed to open Keychain:", e);
    }
  }

  interface ProxyConfigInfo {
    caddyfile_path: string;
    caddyfile_content: string | null;
    plist_file: string;
    plist_content: string | null;
    daemon_installed: boolean;
    daemon_running: boolean;
    daemon_pid: number | null;
    tld: string;
    caddy_version: string | null;
  }

  // View Config modal state
  let showConfigModal = $state(false);
  let proxyConfig = $state<ProxyConfigInfo | null>(null);
  let loadingConfig = $state(false);
  let configError = $state<string | null>(null);

  async function loadProxyConfig() {
    loadingConfig = true;
    configError = null;
    try {
      proxyConfig = await invoke<ProxyConfigInfo>("get_proxy_config");
      showConfigModal = true;
    } catch (e) {
      configError = `Failed to load proxy config: ${String(e)}`;
    } finally {
      loadingConfig = false;
    }
  }

  interface NetworkStatus {
    dns_running: boolean;
    dns_port: number;
    proxy_running: boolean;
    proxy_port: number;
    resolver_installed: boolean;
    active_routes: { domain: string; port: number; instance_id: string }[];
    tld: string;
  }

  interface ProxyStatus {
    daemon_installed: boolean;
    daemon_running: boolean;
    daemon_pid: number | null;
    caddy_installed: boolean;
  }

  interface CliStatus {
    installed: boolean;
    path: string | null;
    binary_exists: boolean;
  }

  interface HelperStatus {
    installed: boolean;
    running: boolean;
  }

  interface CATrustStatus {
    ca_exists: boolean;
    is_trusted: boolean;
    ca_path: string;
    cert_name: string | null;
    cert_expiry: string | null;
  }

  let {
    networkStatus,
    proxyStatus,
    cliStatus = null,
    helperStatus = null,
    caTrustStatus = null,
    // Loading states
    dnsServerAction = false,
    installingResolver = false,
    settingUpProxy = false,
    disablingProxy = false,
    startingDaemon = false,
    restartingDaemon = false,
    installingCli = false,
    installingHelper = false,
    // Event handlers
    onStartDns,
    onStopDns,
    onRestartDns,
    onInstallResolver,
    onUninstallResolver,
    onSetupProxy,
    onDisableProxy,
    onStartDaemon,
    onRestartDaemon,
    onOpenSettings,
    onInstallCli,
    onUninstallCli,
    onInstallHelper,
    onUninstallHelper,
    onRefresh,
    onTrustCA,
    trustingCA = false,
  }: {
    networkStatus: NetworkStatus | null;
    proxyStatus: ProxyStatus | null;
    cliStatus?: CliStatus | null;
    helperStatus?: HelperStatus | null;
    caTrustStatus?: CATrustStatus | null;
    dnsServerAction?: boolean;
    installingResolver?: boolean;
    settingUpProxy?: boolean;
    disablingProxy?: boolean;
    startingDaemon?: boolean;
    restartingDaemon?: boolean;
    installingCli?: boolean;
    installingHelper?: boolean;
    onStartDns: () => void;
    onStopDns: () => void;
    onRestartDns: () => void;
    onInstallResolver: () => void;
    onUninstallResolver: () => void;
    onSetupProxy: () => void;
    onDisableProxy: () => void;
    onStartDaemon: () => void;
    onRestartDaemon: () => void;
    onOpenSettings: () => void;
    onInstallCli: () => void;
    onUninstallCli: () => void;
    onInstallHelper: () => void;
    onUninstallHelper: () => void;
    onRefresh: () => void;
    onTrustCA: () => void;
    trustingCA?: boolean;
  } = $props();
</script>

<div class="general-section">
  <div class="section-header">
    <div class="title-row">
      <h2>General Settings</h2>
      <button class="refresh-btn" onclick={onRefresh} title="Refresh">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 2v6h-6"></path>
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
          <path d="M3 22v-6h6"></path>
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
        </svg>
      </button>
    </div>
  </div>

  {#if networkStatus}
    <section class="card">
      <h3>Network Services</h3>
      <div class="network-grid">
        <div class="network-item">
          <span class="network-label">DNS Server</span>
          <span class="network-value">
            {#if networkStatus.dns_running}
              <span class="status-badge running">Port {networkStatus.dns_port}</span>
              <button
                class="small-button"
                onclick={onRestartDns}
                disabled={dnsServerAction}
              >
                {dnsServerAction ? "..." : "Restart"}
              </button>
              <button
                class="small-button danger"
                onclick={onStopDns}
                disabled={dnsServerAction}
              >
                {dnsServerAction ? "..." : "Stop"}
              </button>
            {:else}
              <span class="status-badge stopped">Stopped</span>
              <button
                class="small-button"
                onclick={onStartDns}
                disabled={dnsServerAction}
              >
                {dnsServerAction ? "Starting..." : "Start"}
              </button>
            {/if}
          </span>
        </div>
        <div class="network-item">
          <span class="network-label">macOS Resolver</span>
          <span class="network-value">
            {#if networkStatus.resolver_installed}
              <span class="status-badge installed">.{networkStatus.tld} enabled</span>
              <button
                class="btn small secondary"
                onclick={onOpenSettings}
              >
                Change TLD
              </button>
              <button
                class="btn small danger-outline"
                onclick={onUninstallResolver}
                disabled={installingResolver}
              >
                Uninstall
              </button>
            {:else}
              <span class="status-badge not-installed">Not Installed</span>
              <button
                class="btn small secondary"
                onclick={onOpenSettings}
              >
                Set TLD
              </button>
              <button
                class="btn small primary"
                onclick={onInstallResolver}
                disabled={installingResolver}
              >
                {installingResolver ? "Installing..." : "Install"}
              </button>
            {/if}
          </span>
        </div>
      </div>

      {#if !networkStatus.resolver_installed}
        <p class="network-hint">
          Install the resolver to access services via <code>.{networkStatus.tld}</code> domains.
        </p>
      {/if}
    </section>

    <!-- Reverse Proxy Section -->
    <section class="card">
      <div class="card-header">
        <h3>Reverse Proxy</h3>
        <button
          class="btn small secondary"
          onclick={loadProxyConfig}
          disabled={loadingConfig}
          title="View proxy configuration"
        >
          {loadingConfig ? "Loading..." : "View Config"}
        </button>
      </div>
      {#if configError}
        <div class="error-banner">
          {configError}
          <button class="dismiss" onclick={() => (configError = null)}>&times;</button>
        </div>
      {/if}
      <div class="https-grid">
        <div class="network-item">
          <span class="network-label">Caddy Daemon</span>
          <span class="network-value">
            {#if proxyStatus?.daemon_installed}
              {#if proxyStatus.daemon_running}
                <span class="status-badge running">Running{#if proxyStatus.daemon_pid} (PID {proxyStatus.daemon_pid}){/if}</span>
              {:else}
                <span class="status-badge stopped">Installed (Not Running)</span>
              {/if}
              <span class="button-group">
                {#if !proxyStatus.daemon_running}
                  <button
                    class="btn small primary"
                    onclick={onStartDaemon}
                    disabled={startingDaemon || restartingDaemon}
                  >
                    {startingDaemon ? "Starting..." : "Start"}
                  </button>
                {:else}
                  <button
                    class="btn small secondary"
                    onclick={onRestartDaemon}
                    disabled={startingDaemon || restartingDaemon}
                  >
                    {restartingDaemon ? "Restarting..." : "Restart"}
                  </button>
                {/if}
                <button
                  class="btn small danger-outline"
                  onclick={onDisableProxy}
                  disabled={disablingProxy || startingDaemon || restartingDaemon}
                >
                  {disablingProxy ? "Disabling..." : "Disable"}
                </button>
              </span>
            {:else}
              <span class="status-badge not-installed">Not Installed</span>
              <button
                class="btn small primary"
                onclick={onSetupProxy}
                disabled={settingUpProxy}
              >
                {settingUpProxy ? "Setting up..." : "Enable Proxy"}
              </button>
            {/if}
          </span>
        </div>
      </div>
      {#if !proxyStatus?.daemon_installed}
        <p class="network-hint">
          Enable the reverse proxy to access services via <code>http://my-service.{networkStatus.tld}</code> (requires admin password).
        </p>
      {/if}
    </section>

    <!-- HTTPS Certificate Section -->
    <section class="card">
      <h3>HTTPS Certificate</h3>
      <div class="network-grid">
        <div class="network-item">
          <span class="network-label">Caddy Root CA</span>
          <span class="network-value">
            {#if caTrustStatus}
              {#if !caTrustStatus.ca_exists}
                <span class="status-badge not-installed">Not Generated</span>
              {:else if caTrustStatus.is_trusted}
                <span class="status-badge running">Trusted</span>
              {:else}
                <span class="status-badge stopped">Not Trusted</span>
              {/if}
            {:else}
              <span class="status-badge not-installed">Loading...</span>
            {/if}
          </span>
        </div>
        {#if caTrustStatus?.ca_exists && caTrustStatus.cert_name}
          <div class="network-item">
            <span class="network-label">Certificate</span>
            <span class="network-value cert-info">{caTrustStatus.cert_name}</span>
          </div>
        {/if}
        {#if caTrustStatus?.ca_exists && caTrustStatus.cert_expiry}
          <div class="network-item">
            <span class="network-label">Expires</span>
            <span class="network-value cert-info">{caTrustStatus.cert_expiry}</span>
          </div>
        {/if}
      </div>
      {#if caTrustStatus && !caTrustStatus.ca_exists}
        <p class="network-hint">
          Enable SSL on a domain and visit it via HTTPS to generate the certificate.
        </p>
      {:else if caTrustStatus && !caTrustStatus.is_trusted}
        <div class="network-hint warning">
          <p>The certificate needs to be trusted for HTTPS to work without warnings.</p>
          <div class="trust-actions">
            <button
              class="btn small primary"
              onclick={onTrustCA}
              disabled={trustingCA}
            >
              {trustingCA ? "Trusting..." : "Trust Certificate"}
            </button>
          </div>
          <details class="manual-trust">
            <summary>Manual instructions</summary>
            <ol class="trust-steps">
              <li>Open Keychain Access</li>
              <li>Find "<strong>{caTrustStatus.cert_name || 'Burd CA Self Signed CN'}</strong>" in System keychain</li>
              <li>Double-click it, expand "Trust", set "Always Trust"</li>
            </ol>
            <button class="btn small secondary" onclick={() => openKeychain()}>
              Open Keychain Access
            </button>
          </details>
        </div>
      {/if}
    </section>

    <!-- CLI Section -->
    <section class="card">
      <h3>Command Line Tool</h3>
      <div class="network-grid">
        <div class="network-item">
          <span class="network-label">Burd CLI</span>
          <span class="network-value">
            {#if cliStatus?.installed}
              <span class="status-badge installed">Installed</span>
              {#if cliStatus.path}
                <span class="cli-path">{cliStatus.path}</span>
              {/if}
              <button
                class="btn small danger-outline"
                onclick={onUninstallCli}
                disabled={installingCli}
              >
                {installingCli ? "..." : "Uninstall"}
              </button>
            {:else}
              <span class="status-badge not-installed">Not Installed</span>
              <button
                class="btn small primary"
                onclick={onInstallCli}
                disabled={installingCli || !cliStatus?.binary_exists}
              >
                {installingCli ? "Installing..." : "Install CLI"}
              </button>
            {/if}
          </span>
        </div>
      </div>
      {#if !cliStatus?.installed}
        <p class="network-hint">
          Install the CLI to quickly create projects from your terminal. Run <code>burd init</code> in any folder to set up a local server.
        </p>
      {:else}
        <p class="network-hint success">
          CLI installed! Run <code>burd init</code> in any project folder to create a local server.
        </p>
      {/if}
    </section>

    <!-- MCP Server Section -->
    <section class="card">
      <h3>MCP Server</h3>
      <div class="network-grid">
        <div class="network-item">
          <span class="network-label">Model Context Protocol</span>
          <span class="network-value">
            {#if cliStatus?.installed}
              <span class="status-badge installed">Available</span>
            {:else}
              <span class="status-badge not-installed">CLI Required</span>
            {/if}
          </span>
        </div>
      </div>
      <div class="mcp-info">
        <p>Allow AI assistants to manage your Burd services programmatically via the Model Context Protocol.</p>

        <details class="mcp-details">
          <summary>Claude Desktop</summary>
          <p class="config-label">Add to <code>~/Library/Application Support/Claude/claude_desktop_config.json</code>:</p>
          <pre class="mcp-config">{`{
  "mcpServers": {
    "burd": {
      "command": "${cliStatus?.path || '/usr/local/bin/burd'}",
      "args": ["mcp"]
    }
  }
}`}</pre>
        </details>

        <details class="mcp-details">
          <summary>Claude Code (CLI)</summary>
          <p class="config-label">Run this command:</p>
          <pre class="mcp-config">{`claude mcp add-json burd '{"command":"${cliStatus?.path || '/usr/local/bin/burd'}","args":["mcp"]}'`}</pre>
          <p class="config-label" style="margin-top: 0.5rem;">Or add to <code>~/.claude/settings.local.json</code>:</p>
          <pre class="mcp-config">{`{
  "mcpServers": {
    "burd": {
      "command": "${cliStatus?.path || '/usr/local/bin/burd'}",
      "args": ["mcp"]
    }
  }
}`}</pre>
        </details>

        <details class="mcp-details">
          <summary>Cursor</summary>
          <p class="config-label">Add to <code>~/.cursor/mcp.json</code>:</p>
          <pre class="mcp-config">{`{
  "mcpServers": {
    "burd": {
      "command": "${cliStatus?.path || '/usr/local/bin/burd'}",
      "args": ["mcp"]
    }
  }
}`}</pre>
        </details>

        <details class="mcp-details">
          <summary>VS Code + Copilot</summary>
          <p class="config-label">Add to VS Code settings or <code>.copilot/mcp-config.json</code>:</p>
          <pre class="mcp-config">{`{
  "mcpServers": {
    "burd": {
      "command": "${cliStatus?.path || '/usr/local/bin/burd'}",
      "args": ["mcp"]
    }
  }
}`}</pre>
        </details>

        <details class="mcp-details">
          <summary>Available Tools</summary>
          <div class="tools-grid">
            <div class="tool-category">
              <span class="category-name">Instances</span>
              <span class="category-tools">list, create, start, stop, restart, delete, logs, env</span>
            </div>
            <div class="tool-category">
              <span class="category-name">Domains</span>
              <span class="category-tools">list, create, update, delete, toggle SSL</span>
            </div>
            <div class="tool-category">
              <span class="category-name">Databases</span>
              <span class="category-tools">list, create, drop</span>
            </div>
            <div class="tool-category">
              <span class="category-name">Services</span>
              <span class="category-tools">list types, get versions</span>
            </div>
            <div class="tool-category">
              <span class="category-name">Status</span>
              <span class="category-tools">system status</span>
            </div>
          </div>
        </details>
      </div>
      {#if !cliStatus?.installed}
        <p class="network-hint warning">
          Install the CLI above to enable MCP server functionality.
        </p>
      {/if}
    </section>

    <!-- Privileged Helper Section -->
    <section class="card">
      <h3>Privileged Helper</h3>
      <div class="network-grid">
        <div class="network-item">
          <span class="network-label">Helper Status</span>
          <span class="network-value">
            {#if helperStatus?.installed}
              {#if helperStatus.running}
                <span class="status-badge running">Running</span>
              {:else}
                <span class="status-badge stopped">Installed (Not Running)</span>
              {/if}
              <button
                class="btn small danger-outline"
                onclick={onUninstallHelper}
                disabled={installingHelper}
              >
                {installingHelper ? "..." : "Uninstall"}
              </button>
            {:else}
              <span class="status-badge not-installed">Not Installed</span>
              <button
                class="btn small primary"
                onclick={onInstallHelper}
                disabled={installingHelper}
              >
                {installingHelper ? "Installing..." : "Install Helper"}
              </button>
            {/if}
          </span>
        </div>
      </div>
      {#if !helperStatus?.installed}
        <p class="network-hint">
          Install the privileged helper to avoid repeated password prompts when starting/stopping services.
          Requires one-time admin password.
        </p>
      {:else if helperStatus?.running}
        <p class="network-hint success">
          Helper running! No password prompts needed for service operations.
        </p>
      {/if}
    </section>
  {:else}
    <div class="loading">Loading network status...</div>
  {/if}
</div>

{#if showConfigModal && proxyConfig}
  <div class="modal-overlay" onclick={() => showConfigModal = false} onkeydown={(e) => e.key === 'Escape' && (showConfigModal = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-content" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Reverse Proxy Configuration</h3>
        <button class="close-btn" onclick={() => showConfigModal = false}>&times;</button>
      </div>
      <div class="modal-body">
        <div class="config-section">
          <h4>Status</h4>
          <div class="config-grid">
            <span class="label">Daemon Installed:</span>
            <span class={proxyConfig.daemon_installed ? "status-ok" : "status-warn"}>
              {proxyConfig.daemon_installed ? "Yes" : "No"}
            </span>
            <span class="label">Daemon Running:</span>
            <span class={proxyConfig.daemon_running ? "status-ok" : "status-warn"}>
              {proxyConfig.daemon_running ? "Yes" : "No"}
            </span>
            <span class="label">TLD:</span>
            <span>.{proxyConfig.tld}</span>
          </div>
        </div>

        <div class="config-section">
          <h4>Paths</h4>
          <div class="config-grid">
            <span class="label">Plist File:</span>
            <code class="path">{proxyConfig.plist_file}</code>
            <span class="label">Caddyfile:</span>
            <code class="path">{proxyConfig.caddyfile_path}</code>
            {#if proxyConfig.caddy_version}
              <span class="label">Caddy Version:</span>
              <span>{proxyConfig.caddy_version}</span>
            {/if}
          </div>
        </div>

        {#if proxyConfig.plist_content}
          <div class="config-section">
            <h4>Daemon Configuration (plist)</h4>
            <pre class="json-content">{proxyConfig.plist_content}</pre>
          </div>
        {/if}

        {#if proxyConfig.caddyfile_content}
          <div class="config-section">
            <h4>Caddyfile Configuration</h4>
            <pre class="json-content">{proxyConfig.caddyfile_content}</pre>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .general-section {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .section-header h2 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .refresh-btn {
    background: none;
    border: none;
    padding: 0.375rem;
    border-radius: 6px;
    cursor: pointer;
    color: #86868b;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  @media (prefers-color-scheme: dark) {
    .refresh-btn {
      color: #98989d;
    }

    .refresh-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
  }

  .card {
    background: white;
    border-radius: 12px;
    padding: 1.5rem;
    border: 1px solid #e5e5e5;
  }

  .card h3 {
    margin: 0 0 1rem;
    font-size: 1.125rem;
    font-weight: 600;
  }

  .network-grid, .https-grid {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .network-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    background: #f5f5f7;
    border-radius: 8px;
  }

  @media (prefers-color-scheme: dark) {
    .network-item {
      background: #1c1c1e;
    }
  }

  .network-label {
    font-weight: 500;
    color: #86868b;
    font-size: 0.875rem;
  }

  .network-value {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .button-group {
    display: flex;
    gap: 0.5rem;
  }

  .status-badge {
    display: inline-block;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .status-badge.installed,
  .status-badge.running {
    background: #dcfce7;
    color: #166534;
  }

  .status-badge.stopped {
    background: #f3f4f6;
    color: #6b7280;
  }

  .status-badge.not-installed {
    background: #f3f4f6;
    color: #6b7280;
  }

  @media (prefers-color-scheme: dark) {
    .status-badge.installed,
    .status-badge.running {
      background: #14532d;
      color: #86efac;
    }

    .status-badge.stopped,
    .status-badge.not-installed {
      background: #27272a;
      color: #a1a1aa;
    }
  }

  .small-button {
    padding: 0.25rem 0.5rem;
    border: none;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 500;
    cursor: pointer;
    background: #e5e5e5;
    color: #1d1d1f;
    transition: background 0.15s ease;
  }

  .small-button:hover:not(:disabled) {
    background: #d1d1d6;
  }

  .small-button.danger {
    background: #ff3b30;
    color: white;
  }

  .small-button.danger:hover:not(:disabled) {
    background: #d63027;
  }

  .small-button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  @media (prefers-color-scheme: dark) {
    .small-button {
      background: #3a3a3c;
      color: #f5f5f7;
    }

    .small-button:hover:not(:disabled) {
      background: #48484a;
    }
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
  }

  .btn.small {
    padding: 0.25rem 0.5rem;
    font-size: 0.75rem;
  }

  .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
  }

  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn.secondary {
    background: #e5e5e5;
    color: #1d1d1f;
  }

  .btn.secondary:hover:not(:disabled) {
    background: #d1d1d6;
  }

  .btn.danger-outline {
    background: transparent;
    color: #ff3b30;
    border: 1px solid #ff3b30;
  }

  .btn.danger-outline:hover:not(:disabled) {
    background: #ff3b30;
    color: white;
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  @media (prefers-color-scheme: dark) {
    .btn.secondary {
      background: #3a3a3c;
      color: #f5f5f7;
    }

    .btn.secondary:hover:not(:disabled) {
      background: #48484a;
    }
  }

  .network-hint {
    margin: 1rem 0 0;
    font-size: 0.875rem;
    color: #86868b;
    background: #f5f5f7;
    padding: 0.75rem;
    border-radius: 6px;
  }

  .network-hint.success {
    background: rgba(52, 199, 89, 0.1);
    color: #34c759;
  }

  .network-hint.warning {
    background: rgba(255, 149, 0, 0.1);
    color: #ff9500;
  }

  .network-hint code {
    background: rgba(0, 0, 0, 0.05);
    padding: 0.125rem 0.25rem;
    border-radius: 3px;
    font-size: 0.8125rem;
  }

  .network-hint p {
    margin: 0 0 0.5rem 0;
  }

  .trust-steps {
    margin: 0.5rem 0;
    padding-left: 1.25rem;
    font-size: 0.8125rem;
  }

  .trust-steps li {
    margin-bottom: 0.25rem;
  }

  @media (prefers-color-scheme: dark) {
    .network-hint {
      background: #1c1c1e;
    }

    .network-hint code {
      background: rgba(255, 255, 255, 0.1);
    }
  }

  .cli-path {
    font-size: 0.75rem;
    color: #86868b;
    font-family: monospace;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .loading {
    text-align: center;
    color: #86868b;
    padding: 2rem;
  }

  /* Light mode explicit overrides */
  :global(:root[data-theme="light"]) .card {
    background: white !important;
  }

  :global(:root[data-theme="light"]) .network-item {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="light"]) .small-button {
    background: #e5e5e5 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .btn.secondary {
    background: #e5e5e5 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .network-hint {
    background: #f5f5f7 !important;
    color: #86868b !important;
  }

  :global(:root[data-theme="light"]) .network-hint code {
    background: rgba(0, 0, 0, 0.05) !important;
  }

  :global(:root[data-theme="light"]) .refresh-btn {
    color: #86868b !important;
  }
  :global(:root[data-theme="light"]) .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }

  /* Dark mode explicit overrides */
  :global(:root[data-theme="dark"]) .card {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .network-item {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .small-button {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .network-hint {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .network-hint code {
    background: rgba(255, 255, 255, 0.1) !important;
  }

  :global(:root[data-theme="dark"]) .network-hint.success {
    background: rgba(52, 199, 89, 0.15) !important;
    color: #4ade80 !important;
  }

  :global(:root[data-theme="dark"]) .network-hint.warning {
    background: rgba(255, 149, 0, 0.15) !important;
    color: #ffb340 !important;
  }

  :global(:root[data-theme="dark"]) .refresh-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }

  /* Light mode status badge overrides */
  :global(:root[data-theme="light"]) .status-badge.installed,
  :global(:root[data-theme="light"]) .status-badge.running {
    background: #dcfce7 !important;
    color: #166534 !important;
  }

  :global(:root[data-theme="light"]) .status-badge.stopped,
  :global(:root[data-theme="light"]) .status-badge.not-installed {
    background: #f3f4f6 !important;
    color: #6b7280 !important;
  }

  /* Dark mode status badge overrides */
  :global(:root[data-theme="dark"]) .status-badge.installed,
  :global(:root[data-theme="dark"]) .status-badge.running {
    background: #14532d !important;
    color: #86efac !important;
  }

  :global(:root[data-theme="dark"]) .status-badge.stopped,
  :global(:root[data-theme="dark"]) .status-badge.not-installed {
    background: #27272a !important;
    color: #a1a1aa !important;
  }

  /* Card header with button */
  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .card-header h3 {
    margin: 0;
  }

  /* Error banner */
  .error-banner {
    background: #fee2e2;
    color: #dc2626;
    padding: 0.75rem 1rem;
    border-radius: 6px;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .error-banner .dismiss {
    background: none;
    border: none;
    color: inherit;
    font-size: 1.25rem;
    cursor: pointer;
  }

  @media (prefers-color-scheme: dark) {
    .error-banner {
      background: #3d2020;
      color: #fca5a5;
    }
  }

  /* Modal styles */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(4px);
  }

  .modal-content {
    background: white;
    border-radius: 12px;
    width: 90%;
    max-width: 700px;
    max-height: 80vh;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
  }

  @media (prefers-color-scheme: dark) {
    .modal-content {
      background: #2c2c2e;
    }
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .modal-header {
      border-bottom-color: #48484a;
    }
  }

  .modal-header h3 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.5rem;
    color: #86868b;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }

  .close-btn:hover {
    color: #1d1d1f;
  }

  @media (prefers-color-scheme: dark) {
    .close-btn:hover {
      color: #f5f5f7;
    }
  }

  .modal-body {
    padding: 1.5rem;
    overflow-y: auto;
  }

  .config-section {
    margin-bottom: 1.5rem;
  }

  .config-section:last-child {
    margin-bottom: 0;
  }

  .config-section h4 {
    margin: 0 0 0.75rem;
    font-size: 0.875rem;
    font-weight: 600;
    color: #86868b;
    text-transform: uppercase;
  }

  .config-grid {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.5rem 1rem;
    align-items: center;
  }

  .config-grid .label {
    font-weight: 500;
    color: #86868b;
    font-size: 0.875rem;
  }

  .config-grid .path {
    background: #f5f5f7;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    word-break: break-all;
  }

  @media (prefers-color-scheme: dark) {
    .config-grid .path {
      background: #1c1c1e;
    }
  }

  .status-ok {
    color: #22c55e;
    font-weight: 500;
  }

  .status-warn {
    color: #f59e0b;
    font-weight: 500;
  }

  .json-content {
    background: #f5f5f7;
    padding: 1rem;
    border-radius: 8px;
    font-size: 0.75rem;
    font-family: 'SF Mono', Menlo, Monaco, 'Courier New', monospace;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 300px;
    overflow-y: auto;
    margin: 0;
  }

  @media (prefers-color-scheme: dark) {
    .json-content {
      background: #1c1c1e;
    }
  }

  /* Theme overrides for modal */
  :global(:root[data-theme="light"]) .modal-content {
    background: white !important;
  }

  :global(:root[data-theme="dark"]) .modal-content {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .modal-header {
    border-bottom-color: #48484a !important;
  }

  :global(:root[data-theme="light"]) .config-grid .path,
  :global(:root[data-theme="light"]) .json-content {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .config-grid .path,
  :global(:root[data-theme="dark"]) .json-content {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .error-banner {
    background: #3d2020 !important;
    color: #fca5a5 !important;
  }

  /* MCP Section styles */
  .mcp-info {
    margin-top: 1rem;
  }

  .mcp-info > p {
    font-size: 0.875rem;
    color: #86868b;
    margin: 0 0 1rem 0;
  }

  .mcp-details {
    margin-bottom: 0.75rem;
    background: #f5f5f7;
    border-radius: 8px;
    overflow: hidden;
  }

  .mcp-details summary {
    padding: 0.75rem 1rem;
    font-weight: 500;
    font-size: 0.875rem;
    cursor: pointer;
    user-select: none;
  }

  .mcp-details summary:hover {
    background: rgba(0, 0, 0, 0.03);
  }

  .mcp-details[open] summary {
    border-bottom: 1px solid #e5e5e5;
  }

  .config-label {
    padding: 0.75rem 1rem 0.5rem;
    font-size: 0.8125rem;
    color: #86868b;
    margin: 0;
  }

  .config-label code {
    background: rgba(0, 0, 0, 0.05);
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.75rem;
  }

  .mcp-config {
    margin: 0;
    padding: 1rem;
    background: #1c1c1e;
    color: #e5e5e7;
    font-size: 0.75rem;
    font-family: 'SF Mono', Menlo, Monaco, 'Courier New', monospace;
    overflow-x: auto;
    white-space: pre;
    border-radius: 0;
  }

  .tools-grid {
    padding: 0.75rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .tool-category {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    font-size: 0.8125rem;
    gap: 1rem;
  }

  .category-name {
    font-weight: 500;
    color: #1d1d1f;
    flex-shrink: 0;
  }

  .category-tools {
    color: #86868b;
    text-align: right;
  }

  @media (prefers-color-scheme: dark) {
    .mcp-details {
      background: #1c1c1e;
    }

    .mcp-details summary:hover {
      background: rgba(255, 255, 255, 0.05);
    }

    .mcp-details[open] summary {
      border-bottom-color: #38383a;
    }

    .config-label code {
      background: rgba(255, 255, 255, 0.1);
    }

    .mcp-config {
      background: #0d0d0d;
    }

    .category-name {
      color: #f5f5f7;
    }
  }

  /* Light mode MCP overrides */
  :global(:root[data-theme="light"]) .mcp-details {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="light"]) .mcp-details summary:hover {
    background: rgba(0, 0, 0, 0.03) !important;
  }

  :global(:root[data-theme="light"]) .mcp-details[open] summary {
    border-bottom-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"]) .config-label code {
    background: rgba(0, 0, 0, 0.05) !important;
  }

  :global(:root[data-theme="light"]) .mcp-config {
    background: #1c1c1e !important;
    color: #e5e5e7 !important;
  }

  :global(:root[data-theme="light"]) .category-name {
    color: #1d1d1f !important;
  }

  /* Dark mode MCP overrides */
  :global(:root[data-theme="dark"]) .mcp-details {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .mcp-details summary:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="dark"]) .mcp-details[open] summary {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .config-label code {
    background: rgba(255, 255, 255, 0.1) !important;
  }

  :global(:root[data-theme="dark"]) .mcp-config {
    background: #0d0d0d !important;
  }

  :global(:root[data-theme="dark"]) .category-name {
    color: #f5f5f7 !important;
  }
</style>
