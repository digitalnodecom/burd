<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";

  interface PHPVersion {
    version: string;
    is_current: boolean;
    is_default: boolean;
  }

  interface CurrentPHP {
    version: string;
    source: string;
    path: string;
    extensions?: string[];
  }

  interface PvmStatus {
    managed_versions: string[];
    default_version: string | null;
    current_php: CurrentPHP | null;
    burd_php: CurrentPHP | null;
  }

  interface ShellConflict {
    overriding_source: string;
    overriding_path: string;
  }

  interface ShellIntegrationStatus {
    configured: boolean;
    profile_path: string | null;
    conflict: ShellConflict | null;
  }

  interface RemotePHPVersion {
    version: string;
    minor_version: string;
    download_url: string;
    size_bytes: number | null;
    is_latest_patch: boolean;
  }

  let pvmStatus = $state<PvmStatus | null>(null);
  let shellStatus = $state<ShellIntegrationStatus | null>(null);
  let installedVersions = $state<PHPVersion[]>([]);
  let remoteVersions = $state<RemotePHPVersion[]>([]);
  let loading = $state(false);
  let loadingRemote = $state(false);
  let error = $state<string | null>(null);
  let downloadingVersion = $state<string | null>(null);
  let downloadProgress = $state<number>(0);
  let deletingVersion = $state<string | null>(null);
  let settingDefault = $state<string | null>(null);
  let configuringShell = $state(false);
  let fixingShell = $state(false);
  let showInstallDialog = $state(false);
  let showShellWarning = $state(false);

  let unlistenProgress: (() => void) | null = null;

  async function loadPvmStatus() {
    try {
      pvmStatus = await invoke<PvmStatus>("get_pvm_status");
    } catch (e) {
      console.error("Failed to get PVM status:", e);
      pvmStatus = { managed_versions: [], default_version: null, current_php: null, burd_php: null };
    }
  }

  async function loadShellStatus() {
    try {
      shellStatus = await invoke<ShellIntegrationStatus>("get_php_shell_integration_status");
    } catch (e) {
      console.error("Failed to get shell status:", e);
      shellStatus = { configured: false, profile_path: null };
    }
  }

  async function loadInstalledVersions() {
    loading = true;
    error = null;
    try {
      installedVersions = await invoke<PHPVersion[]>("list_installed_php_versions");
    } catch (e) {
      console.error("Failed to list installed versions:", e);
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadRemoteVersions() {
    loadingRemote = true;
    try {
      remoteVersions = await invoke<RemotePHPVersion[]>("list_remote_php_versions");
      // Filter out already installed versions
      const installedSet = new Set(installedVersions.map(v => v.version));
      remoteVersions = remoteVersions.filter(v => !installedSet.has(v.version));
    } catch (e) {
      console.error("Failed to list remote versions:", e);
    } finally {
      loadingRemote = false;
    }
  }

  async function downloadVersion(version: string) {
    downloadingVersion = version;
    downloadProgress = 0;
    error = null;
    try {
      await invoke("download_php_version", { version });
      await loadInstalledVersions();
      await loadPvmStatus();
      await loadRemoteVersions();
      showInstallDialog = false;
    } catch (e) {
      error = String(e);
    } finally {
      downloadingVersion = null;
      downloadProgress = 0;
    }
  }

  async function deleteVersion(version: string) {
    const confirmed = await confirm(
      `Delete PHP ${version}?`,
      { title: "Delete PHP Version", kind: "warning" }
    );
    if (!confirmed) return;

    deletingVersion = version;
    error = null;
    try {
      await invoke("delete_php_version", { version });
      await loadInstalledVersions();
      await loadPvmStatus();
    } catch (e) {
      error = String(e);
    } finally {
      deletingVersion = null;
    }
  }

  async function setDefaultVersion(version: string) {
    settingDefault = version;
    error = null;
    try {
      await invoke("set_default_php_version", { version });
      await loadPvmStatus();
      await loadInstalledVersions();
    } catch (e) {
      error = String(e);
    } finally {
      settingDefault = null;
    }
  }

  function promptShellConfiguration() {
    // If there's a current PHP from a different source, show warning
    if (pvmStatus?.current_php && pvmStatus.current_php.source !== "Burd") {
      showShellWarning = true;
    } else {
      configureShell();
    }
  }

  async function configureShell() {
    configuringShell = true;
    showShellWarning = false;
    error = null;
    try {
      await invoke("configure_php_shell_integration");
      await loadShellStatus();
    } catch (e) {
      error = String(e);
    } finally {
      configuringShell = false;
    }
  }

  async function removeShellIntegration() {
    const confirmed = await confirm(
      "Remove Burd PHP from your shell configuration?",
      { title: "Remove Shell Integration", kind: "warning" }
    );
    if (!confirmed) return;

    configuringShell = true;
    error = null;
    try {
      await invoke("remove_php_shell_integration");
      await loadShellStatus();
    } catch (e) {
      error = String(e);
    } finally {
      configuringShell = false;
    }
  }

  async function fixShellIntegration() {
    fixingShell = true;
    error = null;
    try {
      await invoke("fix_php_shell_integration");
      await loadShellStatus();
      await loadPvmStatus();
    } catch (e) {
      error = String(e);
    } finally {
      fixingShell = false;
    }
  }

  function openInstallDialog() {
    showInstallDialog = true;
    loadRemoteVersions();
  }

  function formatBytes(bytes: number | null): string {
    if (!bytes) return "";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function getSourceColor(source: string): string {
    switch (source) {
      case "Burd": return "#34c759";
      case "Herd Pro": return "#8b5cf6";
      case "Homebrew": return "#f59e0b";
      case "MAMP": return "#3b82f6";
      case "XAMPP": return "#ef4444";
      case "System": return "#6b7280";
      default: return "#86868b";
    }
  }

  // Group remote versions by minor version
  function groupByMinor(versions: RemotePHPVersion[]): Record<string, RemotePHPVersion[]> {
    const groups: Record<string, RemotePHPVersion[]> = {};
    for (const v of versions) {
      if (!groups[v.minor_version]) {
        groups[v.minor_version] = [];
      }
      groups[v.minor_version].push(v);
    }
    return groups;
  }

  async function refresh() {
    await Promise.all([loadPvmStatus(), loadShellStatus(), loadInstalledVersions()]);
  }

  onMount(async () => {
    // Listen for download progress events
    unlistenProgress = await listen<{ version: string; progress: number }>(
      "php-download-progress",
      (event) => {
        if (event.payload.version === downloadingVersion) {
          downloadProgress = event.payload.progress;
        }
      }
    );

    await Promise.all([loadPvmStatus(), loadShellStatus(), loadInstalledVersions()]);
  });

  onDestroy(() => {
    if (unlistenProgress) {
      unlistenProgress();
    }
  });

  let groupedVersions = $derived(groupByMinor(remoteVersions));
  let sortedMinorVersions = $derived(Object.keys(groupedVersions).sort((a, b) => {
    const [aMajor, aMinor] = a.split('.').map(Number);
    const [bMajor, bMinor] = b.split('.').map(Number);
    if (aMajor !== bMajor) return bMajor - aMajor;
    return bMinor - aMinor;
  }));
</script>

<div class="php-section">
  <div class="section-header">
    <div class="title-row">
      <h2>PHP</h2>
      <button class="refresh-btn" onclick={refresh} title="Refresh">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 2v6h-6"></path>
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
          <path d="M3 22v-6h6"></path>
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
        </svg>
      </button>
    </div>
    <button class="btn primary small" onclick={openInstallDialog}>
      + Install Version
    </button>
  </div>

  {#if error}
    <div class="error-banner">
      {error}
      <button class="dismiss" onclick={() => (error = null)}>&times;</button>
    </div>
  {/if}

  <!-- Burd PHP (used by Tinker) -->
  <section class="card">
    <h3>Burd PHP</h3>
    <p class="card-subtitle">Used by Tinker and internal tools</p>
    {#if pvmStatus?.burd_php}
      <div class="current-php">
        <div class="current-php-info">
          <div class="php-version-large">PHP {pvmStatus.burd_php.version}</div>
          <div class="php-source">
            <span class="source-badge" style="background: #007aff">
              Burd
            </span>
          </div>
          <div class="php-path">{pvmStatus.burd_php.path}</div>
          {#if pvmStatus.burd_php.extensions && pvmStatus.burd_php.extensions.length > 0}
            <div class="php-extensions">
              <span class="extensions-label">Extensions:</span>
              <span class="extensions-list">{pvmStatus.burd_php.extensions.join(', ')}</span>
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <div class="no-php">
        <p>No Burd PHP installed. Install a version below.</p>
      </div>
    {/if}
  </section>

  <!-- System PHP (from PATH) -->
  <section class="card">
    <h3>System PHP</h3>
    <p class="card-subtitle">Detected from your terminal PATH</p>
    {#if pvmStatus?.current_php}
      <div class="current-php">
        <div class="current-php-info">
          <div class="php-version-large">PHP {pvmStatus.current_php.version}</div>
          <div class="php-source" style="color: {getSourceColor(pvmStatus.current_php.source)}">
            <span class="source-badge" style="background: {getSourceColor(pvmStatus.current_php.source)}">
              {pvmStatus.current_php.source}
            </span>
          </div>
          <div class="php-path">{pvmStatus.current_php.path}</div>
        </div>
      </div>
    {:else}
      <div class="no-php">
        <p>No PHP detected in your terminal</p>
      </div>
    {/if}
  </section>

  <!-- PATH Configuration -->
  <section class="card">
    <div class="shell-header">
      <div>
        <h3>PATH Configuration</h3>
        <p class="shell-description">
          {#if shellStatus?.configured && shellStatus?.conflict}
            <strong>{shellStatus.conflict.overriding_source}</strong> has overridden Burd PHP in your shell PATH.
          {:else if shellStatus?.configured}
            Burd PHP is configured in your shell. New terminal sessions will use Burd-managed PHP.
          {:else}
            Configure your shell to use Burd-managed PHP versions.
          {/if}
        </p>
      </div>
      <div class="shell-actions">
        {#if shellStatus?.configured && shellStatus?.conflict}
          <span class="conflict-badge">Conflict</span>
          <button
            class="btn primary small"
            onclick={fixShellIntegration}
            disabled={fixingShell}
          >
            {fixingShell ? "Fixing..." : "Fix PATH"}
          </button>
          <button
            class="btn secondary small"
            onclick={removeShellIntegration}
            disabled={configuringShell}
          >
            {configuringShell ? "..." : "Remove"}
          </button>
        {:else if shellStatus?.configured}
          <span class="configured-badge">Configured</span>
          <button
            class="btn secondary small"
            onclick={removeShellIntegration}
            disabled={configuringShell}
          >
            {configuringShell ? "..." : "Remove"}
          </button>
        {:else}
          <button
            class="btn primary small"
            onclick={promptShellConfiguration}
            disabled={configuringShell || installedVersions.length === 0}
            title={installedVersions.length === 0 ? "Install a PHP version first" : ""}
          >
            {configuringShell ? "Configuring..." : "Configure Shell"}
          </button>
        {/if}
      </div>
    </div>
    {#if shellStatus?.configured && shellStatus?.conflict}
      <div class="conflict-details">
        <div class="conflict-info">
          <span class="conflict-label">Overriding tool:</span>
          <span>{shellStatus.conflict.overriding_source}</span>
        </div>
        <div class="conflict-info">
          <span class="conflict-label">Resolved path:</span>
          <code>{shellStatus.conflict.overriding_path}</code>
        </div>
        <p class="conflict-hint">"Fix PATH" moves Burd's PATH export to the end of your shell profile so it takes priority.</p>
      </div>
    {/if}
  </section>

  <!-- Installed Versions -->
  <section class="card">
    <h3>Installed Versions</h3>
    {#if loading}
      <div class="loading">Loading versions...</div>
    {:else if installedVersions.length === 0}
      <div class="empty-state">
        <p>No PHP versions installed yet.</p>
        <button class="btn primary" onclick={openInstallDialog}>
          Install a Version
        </button>
      </div>
    {:else}
      <table class="versions-table">
        <thead>
          <tr>
            <th>Version</th>
            <th>Status</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each installedVersions as version}
            <tr>
              <td>
                <code class="version-name">PHP {version.version}</code>
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
                    onclick={() => deleteVersion(version.version)}
                    disabled={deletingVersion === version.version || version.is_current}
                    title={version.is_current ? "Cannot delete current version" : "Delete"}
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
</div>

<!-- Install Dialog -->
{#if showInstallDialog}
  <div class="dialog-overlay" onclick={() => (showInstallDialog = false)} onkeydown={(e) => e.key === 'Escape' && (showInstallDialog = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="dialog" onclick={(e) => e.stopPropagation()} role="document">
      <div class="dialog-header">
        <h3>Install PHP Version</h3>
        <button class="dialog-close" onclick={() => (showInstallDialog = false)}>&times;</button>
      </div>
      <div class="dialog-content">
        {#if loadingRemote}
          <div class="loading">Loading available versions...</div>
        {:else if remoteVersions.length === 0}
          <div class="empty-state">
            <p>All available versions are already installed!</p>
          </div>
        {:else}
          <p class="dialog-hint">Select a PHP version to install:</p>
          <div class="version-groups">
            {#each sortedMinorVersions as minor}
              <div class="version-group">
                <div class="version-group-header">PHP {minor}</div>
                <div class="version-list">
                  {#each groupedVersions[minor] as version}
                    <div class="version-item">
                      <div class="version-info">
                        <code class="version-name">{version.version}</code>
                        {#if version.is_latest_patch}
                          <span class="latest-badge">Latest</span>
                        {/if}
                        {#if version.size_bytes}
                          <span class="size-info">{formatBytes(version.size_bytes)}</span>
                        {/if}
                      </div>
                      <button
                        class="btn primary small"
                        onclick={() => downloadVersion(version.version)}
                        disabled={downloadingVersion !== null}
                      >
                        {#if downloadingVersion === version.version}
                          {downloadProgress > 0 ? `${Math.round(downloadProgress)}%` : "..."}
                        {:else}
                          Install
                        {/if}
                      </button>
                    </div>
                  {/each}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<!-- Shell Warning Dialog -->
{#if showShellWarning}
  <div class="dialog-overlay" onclick={() => (showShellWarning = false)} onkeydown={(e) => e.key === 'Escape' && (showShellWarning = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="dialog warning-dialog" onclick={(e) => e.stopPropagation()} role="document">
      <div class="dialog-header">
        <h3>Current PHP Configuration</h3>
        <button class="dialog-close" onclick={() => (showShellWarning = false)}>&times;</button>
      </div>
      <div class="dialog-content">
        {#if pvmStatus?.current_php}
          <div class="warning-content">
            <div class="warning-icon">
              <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="10"></circle>
                <path d="M12 16v-4"></path>
                <path d="M12 8h.01"></path>
              </svg>
            </div>
            <p>Your terminal currently uses <strong>PHP {pvmStatus.current_php.version}</strong> from <strong style="color: {getSourceColor(pvmStatus.current_php.source)}">{pvmStatus.current_php.source}</strong></p>
            <p class="path-info">{pvmStatus.current_php.path}</p>
            <div class="warning-details">
              <p>Configuring Burd will:</p>
              <ul>
                <li>Add Burd PHP to your PATH in ~/.zshrc</li>
                <li>Make <code>php</code> command use Burd-managed version</li>
                <li>Your {pvmStatus.current_php.source} PHP will still work within its own apps</li>
              </ul>
            </div>
          </div>
          <div class="dialog-actions">
            <button class="btn secondary" onclick={() => (showShellWarning = false)}>Cancel</button>
            <button class="btn primary" onclick={configureShell} disabled={configuringShell}>
              {configuringShell ? "Configuring..." : "Configure Shell"}
            </button>
          </div>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .php-section {
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
    margin: 0 0 0.25rem;
    font-size: 1rem;
    font-weight: 600;
  }

  .card-subtitle {
    margin: 0 0 1rem;
    font-size: 0.8rem;
    color: #86868b;
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

  /* Current PHP */
  .current-php {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .current-php-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .php-version-large {
    font-size: 1.25rem;
    font-weight: 600;
  }

  .source-badge {
    display: inline-block;
    color: white;
    padding: 0.125rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .php-path {
    font-size: 0.75rem;
    color: #86868b;
    font-family: monospace;
  }

  .php-extensions {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid #e5e5e5;
  }

  .extensions-label {
    font-size: 0.75rem;
    font-weight: 600;
    color: #636366;
    display: block;
    margin-bottom: 4px;
  }

  .extensions-list {
    font-size: 0.7rem;
    color: #86868b;
    font-family: monospace;
    line-height: 1.6;
    display: block;
    word-break: break-word;
  }

  :global(:root[data-theme="dark"]) .php-extensions {
    border-top-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .extensions-label {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .extensions-list {
    color: #86868b;
  }

  .no-php {
    text-align: center;
    padding: 1rem;
    color: #86868b;
  }

  /* Shell Integration */
  .shell-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
  }

  .shell-header h3 {
    margin-bottom: 0.25rem;
  }

  .shell-description {
    margin: 0;
    font-size: 0.875rem;
    color: #86868b;
  }

  .shell-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-shrink: 0;
  }

  .configured-badge {
    background: #dcfce7;
    color: #16a34a;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  @media (prefers-color-scheme: dark) {
    .configured-badge {
      background: #14532d;
    }
  }

  .conflict-badge {
    background: #fef3c7;
    color: #d97706;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  @media (prefers-color-scheme: dark) {
    .conflict-badge {
      background: #78350f;
      color: #fbbf24;
    }
  }

  .conflict-details {
    margin-top: 1rem;
    padding: 0.75rem 1rem;
    background: #fffbeb;
    border: 1px solid #fde68a;
    border-radius: 8px;
    font-size: 0.8125rem;
  }

  @media (prefers-color-scheme: dark) {
    .conflict-details {
      background: #451a0380;
      border-color: #92400e;
    }
  }

  .conflict-info {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.375rem;
  }

  .conflict-label {
    font-weight: 600;
    color: #92400e;
  }

  @media (prefers-color-scheme: dark) {
    .conflict-label {
      color: #fbbf24;
    }
  }

  .conflict-info code {
    font-size: 0.75rem;
    color: #86868b;
  }

  .conflict-hint {
    margin: 0.5rem 0 0;
    font-size: 0.75rem;
    color: #86868b;
  }

  /* Versions Table */
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

  .loading, .empty-state {
    text-align: center;
    padding: 2rem;
    color: #86868b;
  }

  .empty-state p {
    margin: 0 0 1rem;
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

  .btn.secondary {
    background: #f5f5f7;
    color: #1d1d1f;
  }

  @media (prefers-color-scheme: dark) {
    .btn.secondary {
      background: #3a3a3c;
      color: #f5f5f7;
    }
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

  .version-groups {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .version-group-header {
    font-weight: 600;
    font-size: 0.875rem;
    color: #86868b;
    margin-bottom: 0.5rem;
  }

  .version-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
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

  .latest-badge {
    background: #ff6b6b;
    color: white;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .size-info {
    font-size: 0.75rem;
    color: #86868b;
  }

  /* Warning dialog */
  .warning-dialog {
    max-width: 450px;
  }

  .warning-content {
    text-align: center;
  }

  .warning-icon {
    margin-bottom: 1rem;
  }

  .warning-icon svg {
    stroke: #f59e0b;
  }

  .warning-content p {
    margin: 0 0 0.5rem;
  }

  .path-info {
    font-size: 0.75rem;
    color: #86868b;
    font-family: monospace;
    margin-bottom: 1rem !important;
  }

  .warning-details {
    text-align: left;
    background: #f5f5f7;
    border-radius: 8px;
    padding: 1rem;
    margin-top: 1rem;
  }

  @media (prefers-color-scheme: dark) {
    .warning-details {
      background: #1c1c1e;
    }
  }

  .warning-details p {
    margin: 0 0 0.5rem;
    font-weight: 600;
  }

  .warning-details ul {
    margin: 0;
    padding-left: 1.25rem;
    font-size: 0.875rem;
    color: #86868b;
  }

  .warning-details li {
    margin-bottom: 0.25rem;
  }

  .warning-details code {
    background: #e5e5e5;
    padding: 0.125rem 0.25rem;
    border-radius: 3px;
    font-size: 0.8125rem;
  }

  @media (prefers-color-scheme: dark) {
    .warning-details code {
      background: #3a3a3c;
    }
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 1.5rem;
    padding-top: 1rem;
    border-top: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .dialog-actions {
      border-top-color: #38383a;
    }
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

  :global(:root[data-theme="light"]) .btn.secondary {
    background: #f5f5f7 !important;
    color: #1d1d1f !important;
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

  :global(:root[data-theme="light"]) .versions-table th {
    border-bottom-color: #e5e5e5 !important;
  }
  :global(:root[data-theme="light"]) .versions-table td {
    border-bottom-color: #f0f0f0 !important;
  }

  :global(:root[data-theme="light"]) .configured-badge {
    background: #dcfce7 !important;
    color: #16a34a !important;
  }

  :global(:root[data-theme="light"]) .conflict-badge {
    background: #fef3c7 !important;
    color: #d97706 !important;
  }

  :global(:root[data-theme="light"]) .conflict-details {
    background: #fffbeb !important;
    border-color: #fde68a !important;
  }

  :global(:root[data-theme="light"]) .conflict-label {
    color: #92400e !important;
  }

  :global(:root[data-theme="light"]) .versions-table tbody tr:hover {
    background: rgba(0, 0, 0, 0.04) !important;
  }

  /* Dark mode explicit overrides */
  :global(:root[data-theme="dark"]) .card {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
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

  :global(:root[data-theme="dark"]) .versions-table th {
    border-bottom-color: #48484a !important;
  }
  :global(:root[data-theme="dark"]) .versions-table td {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .configured-badge {
    background: #14532d !important;
    color: #86efac !important;
  }

  :global(:root[data-theme="dark"]) .conflict-badge {
    background: #78350f !important;
    color: #fbbf24 !important;
  }

  :global(:root[data-theme="dark"]) .conflict-details {
    background: #451a0380 !important;
    border-color: #92400e !important;
  }

  :global(:root[data-theme="dark"]) .conflict-label {
    color: #fbbf24 !important;
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

  :global(:root[data-theme="dark"]) .versions-table th,
  :global(:root[data-theme="dark"]) .versions-table td {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .dialog {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .dialog-header {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .warning-details {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .warning-details code {
    background: #3a3a3c !important;
  }

  :global(:root[data-theme="dark"]) .error-banner {
    background: #3d2020 !important;
    color: #fca5a5 !important;
  }
</style>
