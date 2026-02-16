<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  interface NodeVersion {
    version: string;
    is_lts: boolean;
    lts_name: string | null;
    is_current: boolean;
    is_default: boolean;
  }

  interface NvmStatus {
    installed: boolean;
    nvm_dir: string | null;
    current_version: string | null;
    default_version: string | null;
  }

  interface Pm2Process {
    pm_id: number;
    name: string;
    status: string;
    memory: number;
    cpu: number;
    uptime: number | null;
    restart_count: number;
    script: string | null;
  }

  interface Pm2Status {
    installed: boolean;
    version: string | null;
    processes: Pm2Process[];
  }

  let nvmStatus = $state<NvmStatus | null>(null);
  let pm2Status = $state<Pm2Status | null>(null);
  let installedVersions = $state<NodeVersion[]>([]);
  let remoteVersions = $state<NodeVersion[]>([]);
  let loading = $state(false);
  let loadingRemote = $state(false);
  let error = $state<string | null>(null);
  let installingVersion = $state<string | null>(null);
  let uninstallingVersion = $state<string | null>(null);
  let settingDefault = $state<string | null>(null);
  let showInstallDialog = $state(false);

  // PM2 state
  let pm2Loading = $state(false);
  let pm2Installing = $state(false);

  async function loadNvmStatus() {
    try {
      nvmStatus = await invoke<NvmStatus>("get_nvm_status");
    } catch (e) {
      console.error("Failed to get NVM status:", e);
      nvmStatus = { installed: false, nvm_dir: null, current_version: null, default_version: null };
    }
  }

  async function loadInstalledVersions() {
    if (!nvmStatus?.installed) return;

    loading = true;
    error = null;
    try {
      installedVersions = await invoke<NodeVersion[]>("list_installed_node_versions");
    } catch (e) {
      console.error("Failed to list installed versions:", e);
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadRemoteVersions() {
    if (!nvmStatus?.installed) return;

    loadingRemote = true;
    try {
      remoteVersions = await invoke<NodeVersion[]>("list_remote_node_versions");
      // Filter out already installed versions
      const installedSet = new Set(installedVersions.map(v => v.version));
      remoteVersions = remoteVersions.filter(v => !installedSet.has(v.version));
    } catch (e) {
      console.error("Failed to list remote versions:", e);
    } finally {
      loadingRemote = false;
    }
  }

  async function installVersion(version: string) {
    installingVersion = version;
    error = null;
    try {
      await invoke("install_node_version", { version });
      await loadInstalledVersions();
      await loadRemoteVersions();
      showInstallDialog = false;
    } catch (e) {
      error = String(e);
    } finally {
      installingVersion = null;
    }
  }

  async function uninstallVersion(version: string) {
    const confirmed = await confirm(
      `Uninstall Node.js ${version}?`,
      { title: "Uninstall Node.js", kind: "warning" }
    );
    if (!confirmed) return;

    uninstallingVersion = version;
    error = null;
    try {
      await invoke("uninstall_node_version", { version });
      await loadInstalledVersions();
    } catch (e) {
      error = String(e);
    } finally {
      uninstallingVersion = null;
    }
  }

  async function setDefaultVersion(version: string) {
    settingDefault = version;
    error = null;
    try {
      await invoke("set_default_node_version", { version });
      await loadNvmStatus();
      await loadInstalledVersions();
    } catch (e) {
      error = String(e);
    } finally {
      settingDefault = null;
    }
  }

  function openInstallDialog() {
    showInstallDialog = true;
    loadRemoteVersions();
  }

  // PM2 Functions
  async function loadPm2Status() {
    pm2Loading = true;
    try {
      pm2Status = await invoke<Pm2Status>("get_pm2_status");
    } catch (e) {
      console.error("Failed to get PM2 status:", e);
      pm2Status = { installed: false, version: null, processes: [] };
    } finally {
      pm2Loading = false;
    }
  }

  async function installPm2() {
    pm2Installing = true;
    error = null;
    try {
      await invoke("install_pm2");
      await loadPm2Status();
    } catch (e) {
      error = String(e);
    } finally {
      pm2Installing = false;
    }
  }

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + " " + sizes[i];
  }

  function formatUptime(ms: number | null): string {
    if (ms === null || ms === 0) return "-";
    const seconds = Math.floor(ms / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (days > 0) return `${days}d ${hours % 24}h`;
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    if (minutes > 0) return `${minutes}m`;
    return `${seconds}s`;
  }

  onMount(async () => {
    await loadNvmStatus();
    if (nvmStatus?.installed) {
      await loadInstalledVersions();
      await loadPm2Status();
    }
  });
</script>

<div class="node-section">
  <div class="section-header">
    <h2>Node.js</h2>
    {#if nvmStatus?.installed}
      <button class="btn primary small" onclick={openInstallDialog}>
        + Install Version
      </button>
    {/if}
  </div>

  {#if error}
    <div class="error-banner">
      {error}
      <button class="dismiss" onclick={() => (error = null)}>&times;</button>
    </div>
  {/if}

  {#if !nvmStatus}
    <section class="card">
      <div class="loading">Checking NVM status...</div>
    </section>
  {:else if !nvmStatus.installed}
    <section class="card">
      <div class="not-installed">
        <div class="not-installed-icon">
          <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <path d="M12 16v-4"></path>
            <path d="M12 8h.01"></path>
          </svg>
        </div>
        <h3>NVM Not Installed</h3>
        <p>
          Node Version Manager (NVM) is required to manage Node.js versions.
        </p>
        <div class="install-instructions">
          <p>Install NVM by running:</p>
          <code class="install-command">
            curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
          </code>
          <p class="hint">Then restart your terminal and reopen Burd.</p>
        </div>
      </div>
    </section>
  {:else}
    <section class="card">
      <div class="status-bar">
        <div class="status-item">
          <span class="status-label">NVM</span>
          <span class="status-value installed">Installed</span>
        </div>
        {#if nvmStatus.current_version}
          <div class="status-item">
            <span class="status-label">Current</span>
            <span class="status-value">{nvmStatus.current_version}</span>
          </div>
        {/if}
        {#if nvmStatus.default_version}
          <div class="status-item">
            <span class="status-label">Default</span>
            <span class="status-value">{nvmStatus.default_version}</span>
          </div>
        {/if}
      </div>
    </section>

    <section class="card">
      <h3>Installed Versions</h3>
      {#if loading}
        <div class="loading">Loading versions...</div>
      {:else if installedVersions.length === 0}
        <div class="empty-state">
          <p>No Node.js versions installed yet.</p>
          <button class="btn primary" onclick={openInstallDialog}>
            Install a Version
          </button>
        </div>
      {:else}
        <table class="versions-table">
          <thead>
            <tr>
              <th>Version</th>
              <th>LTS</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            {#each installedVersions as version}
              <tr>
                <td>
                  <code class="version-name">{version.version}</code>
                </td>
                <td>
                  {#if version.is_lts && version.lts_name}
                    <span class="lts-badge">{version.lts_name}</span>
                  {:else}
                    <span class="no-lts">-</span>
                  {/if}
                </td>
                <td>
                  <div class="status-badges">
                    {#if version.is_current}
                      <span class="badge current">Current</span>
                    {/if}
                    {#if version.is_default}
                      <span class="badge default">Default</span>
                    {/if}
                    {#if !version.is_current && !version.is_default}
                      <span class="badge none">-</span>
                    {/if}
                  </div>
                </td>
                <td>
                  <div class="actions">
                    {#if !version.is_default}
                      <button
                        class="icon-btn default"
                        onclick={() => setDefaultVersion(version.version)}
                        disabled={settingDefault === version.version}
                        title="Set as Default"
                      >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon>
                        </svg>
                      </button>
                    {/if}
                    <button
                      class="icon-btn danger"
                      onclick={() => uninstallVersion(version.version)}
                      disabled={uninstallingVersion === version.version || version.is_current}
                      title={version.is_current ? "Cannot uninstall current version" : "Uninstall"}
                    >
                      <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <polyline points="3 6 5 6 21 6"></polyline>
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                      </svg>
                    </button>
                  </div>
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>

    <!-- PM2 Section -->
    <section class="card pm2-section">
      <div class="pm2-header">
        <h3>PM2 Process Manager</h3>
        {#if pm2Status?.installed}
          <button class="icon-btn" onclick={loadPm2Status} title="Refresh">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="23 4 23 10 17 10"></polyline>
              <polyline points="1 20 1 14 7 14"></polyline>
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
            </svg>
          </button>
        {/if}
      </div>

      {#if pm2Loading}
        <div class="loading">Checking PM2 status...</div>
      {:else if !pm2Status?.installed}
        <div class="pm2-not-installed">
          <p>PM2 is not installed. Install it to manage Node.js processes.</p>
          <button
            class="btn primary"
            onclick={installPm2}
            disabled={pm2Installing}
          >
            {pm2Installing ? "Installing..." : "Install PM2"}
          </button>
          <p class="hint">Installs via: npm install -g pm2</p>
        </div>
      {:else}
        <div class="pm2-status-bar">
          <div class="status-item">
            <span class="status-label">PM2</span>
            <span class="status-value installed">v{pm2Status.version}</span>
          </div>
          <div class="status-item">
            <span class="status-label">Processes</span>
            <span class="status-value">{pm2Status.processes.length}</span>
          </div>
        </div>

        <p class="pm2-hint">PM2 processes started by Burd are managed in the Instances section.</p>

        {#if pm2Status.processes.length === 0}
          <div class="empty-state small">
            <p>No PM2 processes running.</p>
          </div>
        {:else}
          <table class="pm2-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Status</th>
                <th>CPU</th>
                <th>Memory</th>
                <th>Uptime</th>
              </tr>
            </thead>
            <tbody>
              {#each pm2Status.processes as proc}
                <tr>
                  <td>
                    <code class="process-name">{proc.name}</code>
                    <span class="process-id">#{proc.pm_id}</span>
                  </td>
                  <td>
                    <span class="pm2-status {proc.status}">{proc.status}</span>
                  </td>
                  <td>{proc.cpu.toFixed(1)}%</td>
                  <td>{formatBytes(proc.memory)}</td>
                  <td>{formatUptime(proc.uptime)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      {/if}
    </section>
  {/if}
</div>

{#if showInstallDialog}
  <div class="dialog-overlay" onclick={() => (showInstallDialog = false)} onkeydown={(e) => e.key === 'Escape' && (showInstallDialog = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="dialog" onclick={(e) => e.stopPropagation()} role="document">
      <div class="dialog-header">
        <h3>Install Node.js Version</h3>
        <button class="dialog-close" onclick={() => (showInstallDialog = false)}>&times;</button>
      </div>
      <div class="dialog-content">
        {#if loadingRemote}
          <div class="loading">Loading available versions...</div>
        {:else if remoteVersions.length === 0}
          <div class="empty-state">
            <p>All LTS versions are already installed!</p>
          </div>
        {:else}
          <p class="dialog-hint">Select an LTS version to install:</p>
          <div class="version-list">
            {#each remoteVersions as version}
              <div class="version-item">
                <div class="version-info">
                  <code class="version-name">{version.version}</code>
                  {#if version.lts_name}
                    <span class="lts-badge">{version.lts_name}</span>
                  {/if}
                </div>
                <button
                  class="btn primary small"
                  onclick={() => installVersion(version.version)}
                  disabled={installingVersion !== null}
                >
                  {installingVersion === version.version ? "Installing..." : "Install"}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .node-section {
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
    font-size: 1rem;
    font-weight: 600;
  }

  .error-banner {
    background: #fee2e2;
    color: #dc2626;
    padding: 0.75rem 1rem;
    border-radius: 6px;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  @media (prefers-color-scheme: dark) {
    .error-banner {
      background: #3d2020;
      color: #fca5a5;
    }
  }

  .error-banner .dismiss {
    background: none;
    border: none;
    color: inherit;
    font-size: 1.25rem;
    cursor: pointer;
  }

  .status-bar {
    display: flex;
    gap: 2rem;
    flex-wrap: wrap;
  }

  .status-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .status-label {
    font-size: 0.75rem;
    font-weight: 500;
    color: #86868b;
    text-transform: uppercase;
  }

  .status-value {
    font-size: 0.9375rem;
    font-weight: 500;
  }

  .status-value.installed {
    color: #34c759;
  }

  .not-installed,
  .empty-state {
    text-align: center;
    padding: 2rem;
  }

  .not-installed-icon {
    margin-bottom: 1rem;
  }

  .not-installed-icon svg {
    stroke: #ff9500;
  }

  .not-installed h3 {
    margin: 0 0 0.5rem;
    font-size: 1.25rem;
    font-weight: 600;
  }

  .not-installed p,
  .empty-state p {
    margin: 0 0 1rem;
    color: #86868b;
  }

  .install-instructions {
    background: #f5f5f7;
    border-radius: 8px;
    padding: 1rem;
    max-width: 500px;
    margin: 0 auto;
  }

  @media (prefers-color-scheme: dark) {
    .install-instructions {
      background: #1c1c1e;
    }
  }

  .install-instructions p {
    margin: 0 0 0.5rem;
    font-size: 0.875rem;
  }

  .install-instructions .hint {
    margin-top: 0.75rem;
    font-size: 0.8125rem;
    color: #86868b;
  }

  .install-command {
    display: block;
    background: #1c1c1e;
    color: #34c759;
    padding: 0.75rem 1rem;
    border-radius: 6px;
    font-size: 0.75rem;
    overflow-x: auto;
    white-space: nowrap;
    user-select: all;
  }

  @media (prefers-color-scheme: dark) {
    .install-command {
      background: #0c0c0e;
    }
  }

  .versions-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  .versions-table th {
    text-align: left;
    padding: 0.75rem;
    border-bottom: 1px solid #e5e5e5;
    font-weight: 600;
    color: #86868b;
    font-size: 0.75rem;
    text-transform: uppercase;
  }

  .versions-table td {
    padding: 0.75rem;
    border-bottom: 1px solid #f0f0f0;
  }

  @media (prefers-color-scheme: dark) {
    .versions-table th {
      border-bottom-color: #48484a;
    }
    .versions-table td {
      border-bottom-color: #38383a;
    }
  }

  .version-name {
    background: #f5f5f7;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.8125rem;
  }

  @media (prefers-color-scheme: dark) {
    .version-name {
      background: #1c1c1e;
    }
  }

  .lts-badge {
    display: inline-block;
    background: #34c759;
    color: white;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .no-lts {
    color: #86868b;
  }

  .status-badges {
    display: flex;
    gap: 0.375rem;
  }

  .badge {
    display: inline-block;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .badge.current {
    background: #007aff;
    color: white;
  }

  .badge.default {
    background: #ff9500;
    color: white;
  }

  .badge.none {
    background: transparent;
    color: #86868b;
  }

  .actions {
    display: flex;
    gap: 0.25rem;
  }

  /* Icon button styles */
  .icon-btn {
    background: none;
    border: none;
    padding: 0.375rem;
    cursor: pointer;
    color: #86868b;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  .icon-btn.default:hover {
    background: rgba(234, 179, 8, 0.1);
    color: #ca8a04;
  }

  .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1);
    color: #dc2626;
  }

  .icon-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  @media (prefers-color-scheme: dark) {
    .icon-btn {
      color: #98989d;
    }
    .icon-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
    .icon-btn.default:hover {
      background: rgba(234, 179, 8, 0.15);
      color: #facc15;
    }
    .icon-btn.danger:hover {
      background: rgba(239, 68, 68, 0.2);
      color: #ef4444;
    }
  }

  .loading {
    text-align: center;
    padding: 2rem;
    color: #86868b;
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
  }

  .btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
  }

  .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Dialog styles */
  .dialog-overlay {
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
  }

  .dialog {
    background: white;
    border-radius: 12px;
    width: 100%;
    max-width: 500px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
  }

  @media (prefers-color-scheme: dark) {
    .dialog {
      background: #2c2c2e;
    }
  }

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .dialog-header {
      border-bottom-color: #38383a;
    }
  }

  .dialog-header h3 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
  }

  .dialog-close {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: #86868b;
    line-height: 1;
  }

  .dialog-content {
    padding: 1.5rem;
    overflow-y: auto;
  }

  .dialog-hint {
    margin: 0 0 1rem;
    color: #86868b;
    font-size: 0.875rem;
  }

  .version-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .version-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem;
    background: #f5f5f7;
    border-radius: 8px;
  }

  @media (prefers-color-scheme: dark) {
    .version-item {
      background: #1c1c1e;
    }
  }

  .version-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  /* Table row hover */
  .versions-table tbody tr:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  @media (prefers-color-scheme: dark) {
    .versions-table tbody tr:hover {
      background: rgba(255, 255, 255, 0.05);
    }
  }

  /* Light mode explicit overrides */
  :global(:root[data-theme="light"]) .card {
    background: white !important;
  }

  :global(:root[data-theme="light"]) .refresh-btn {
    color: #86868b !important;
  }
  :global(:root[data-theme="light"]) .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .icon-btn {
    color: #86868b !important;
  }
  :global(:root[data-theme="light"]) .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.default:hover {
    background: rgba(234, 179, 8, 0.1) !important;
    color: #ca8a04 !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1) !important;
    color: #dc2626 !important;
  }

  :global(:root[data-theme="light"]) .versions-table tbody tr:hover {
    background: rgba(0, 0, 0, 0.04) !important;
  }

  :global(:root[data-theme="light"]) .versions-table th {
    border-bottom-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"]) .versions-table td {
    border-bottom-color: #f0f0f0 !important;
  }

  :global(:root[data-theme="light"]) .install-instructions {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="light"]) .install-command {
    background: #1c1c1e !important;
    color: #34c759 !important;
  }

  /* Dark mode explicit overrides */
  :global(:root[data-theme="dark"]) .card {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .refresh-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.default:hover {
    background: rgba(234, 179, 8, 0.15) !important;
    color: #facc15 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2) !important;
    color: #ef4444 !important;
  }

  :global(:root[data-theme="dark"]) .versions-table tbody tr:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="dark"]) .version-name {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .version-item {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .versions-table th {
    border-bottom-color: #48484a !important;
  }

  :global(:root[data-theme="dark"]) .versions-table td {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .dialog {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .dialog-header {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .install-instructions {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .install-command {
    background: #0c0c0e !important;
  }

  :global(:root[data-theme="dark"]) .error-banner {
    background: #3d2020 !important;
    color: #fca5a5 !important;
  }

  /* PM2 Section Styles */
  .pm2-section {
    margin-top: 0;
  }

  .pm2-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .pm2-header h3 {
    margin: 0;
    font-size: 1rem;
    font-weight: 600;
  }

  .pm2-not-installed {
    text-align: center;
    padding: 1.5rem;
  }

  .pm2-not-installed p {
    margin: 0 0 1rem;
    color: #86868b;
  }

  .pm2-not-installed .hint {
    margin-top: 0.75rem;
    font-size: 0.75rem;
    color: #86868b;
  }

  .pm2-status-bar {
    display: flex;
    gap: 2rem;
    margin-bottom: 1rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid #e5e5e5;
  }

  .pm2-hint {
    font-size: 0.8125rem;
    color: #86868b;
    margin: 0 0 1rem;
  }

  @media (prefers-color-scheme: dark) {
    .pm2-status-bar {
      border-bottom-color: #38383a;
    }
  }

  .pm2-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8125rem;
  }

  .pm2-table th {
    text-align: left;
    padding: 0.5rem;
    border-bottom: 1px solid #e5e5e5;
    font-weight: 600;
    color: #86868b;
    font-size: 0.6875rem;
    text-transform: uppercase;
  }

  .pm2-table td {
    padding: 0.5rem;
    border-bottom: 1px solid #f0f0f0;
  }

  @media (prefers-color-scheme: dark) {
    .pm2-table th {
      border-bottom-color: #48484a;
    }
    .pm2-table td {
      border-bottom-color: #38383a;
    }
  }

  .pm2-table tbody tr:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  @media (prefers-color-scheme: dark) {
    .pm2-table tbody tr:hover {
      background: rgba(255, 255, 255, 0.05);
    }
  }

  .process-name {
    background: #f5f5f7;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.75rem;
  }

  @media (prefers-color-scheme: dark) {
    .process-name {
      background: #1c1c1e;
    }
  }

  .process-id {
    margin-left: 0.375rem;
    color: #86868b;
    font-size: 0.6875rem;
  }

  .pm2-status {
    display: inline-block;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .pm2-status.online {
    background: rgba(52, 199, 89, 0.15);
    color: #34c759;
  }

  .pm2-status.stopped {
    background: rgba(142, 142, 147, 0.15);
    color: #8e8e93;
  }

  .pm2-status.errored {
    background: rgba(255, 59, 48, 0.15);
    color: #ff3b30;
  }

  .icon-btn.success:hover {
    background: rgba(52, 199, 89, 0.1);
    color: #34c759;
  }

  @media (prefers-color-scheme: dark) {
    .icon-btn.success:hover {
      background: rgba(52, 199, 89, 0.15);
      color: #30d158;
    }
  }

  .empty-state.small {
    padding: 1rem;
  }

  .empty-state.small p {
    margin: 0;
    font-size: 0.875rem;
  }

  .logs-dialog {
    max-width: 800px;
    max-height: 90vh;
  }

  .logs-content {
    background: #1c1c1e;
    color: #f5f5f7;
    padding: 1rem;
    border-radius: 6px;
    font-size: 0.75rem;
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 500px;
    overflow-y: auto;
    margin: 0;
  }

  /* Light/Dark mode explicit overrides for PM2 */
  :global(:root[data-theme="light"]) .pm2-status-bar {
    border-bottom-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"]) .pm2-table th {
    border-bottom-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"]) .pm2-table td {
    border-bottom-color: #f0f0f0 !important;
  }

  :global(:root[data-theme="light"]) .pm2-table tbody tr:hover {
    background: rgba(0, 0, 0, 0.04) !important;
  }

  :global(:root[data-theme="light"]) .process-name {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .pm2-status-bar {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .pm2-table th {
    border-bottom-color: #48484a !important;
  }

  :global(:root[data-theme="dark"]) .pm2-table td {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .pm2-table tbody tr:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="dark"]) .process-name {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn.success:hover {
    background: rgba(52, 199, 89, 0.15) !important;
    color: #30d158 !important;
  }

  :global(:root[data-theme="light"]) .icon-btn.success:hover {
    background: rgba(52, 199, 89, 0.1) !important;
    color: #34c759 !important;
  }
</style>
