<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import type { FrpcPolling } from "$lib/composables/useFrpcPolling.svelte";

  interface FrpServer {
    id: string;
    name: string;
    server_addr: string;
    server_port: number;
    token: string;
    subdomain_host: string;
    is_default: boolean;
    created_at: string;
  }

  interface FrpcInstance {
    id: string;
    running: boolean;
  }

  let {
    server,
    frpcInstance,
    polling,
    expanded = $bindable(false),
    onSave,
    onDelete,
    onStartFrpc,
    onNavigateToServices,
  }: {
    server: FrpServer | null;
    frpcInstance: FrpcInstance | undefined;
    polling: FrpcPolling;
    expanded?: boolean;
    onSave: () => Promise<void>;
    onDelete?: () => Promise<void>;
    onStartFrpc?: (id: string) => void;
    onNavigateToServices?: () => void;
  } = $props();

  // Form state
  let editing = $state(false);
  let serverName = $state("");
  let serverAddr = $state("");
  let serverPort = $state(7000);
  let serverToken = $state("");
  let subdomainHost = $state("");
  let saving = $state(false);
  let error = $state<string | null>(null);

  // Auto-expand and show form if no server configured
  $effect(() => {
    if (!server) {
      expanded = true;
      editing = true;
    }
  });

  function loadForm() {
    if (server) {
      serverName = server.name;
      serverAddr = server.server_addr;
      serverPort = server.server_port;
      serverToken = server.token;
      subdomainHost = server.subdomain_host;
    } else {
      serverName = "";
      serverAddr = "";
      serverPort = 7000;
      serverToken = "";
      subdomainHost = "";
    }
    editing = true;
  }

  function cancelEdit() {
    editing = false;
    error = null;
    if (!server) {
      expanded = false;
    }
  }

  async function generateToken() {
    try {
      serverToken = await invoke<string>("generate_server_token");
    } catch (e) {
      error = String(e);
    }
  }

  async function save() {
    if (!serverName.trim() || !serverAddr.trim() || !serverToken.trim() || !subdomainHost.trim()) {
      error = "All fields are required";
      return;
    }

    saving = true;
    error = null;
    try {
      if (server) {
        await invoke("update_frp_server", {
          id: server.id,
          name: serverName.trim(),
          serverAddr: serverAddr.trim(),
          serverPort,
          token: serverToken,
          subdomainHost: subdomainHost.trim(),
        });
      } else {
        await invoke("create_frp_server", {
          name: serverName.trim(),
          serverAddr: serverAddr.trim(),
          serverPort,
          token: serverToken,
          subdomainHost: subdomainHost.trim(),
        });
      }
      editing = false;
      await onSave();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  async function handleDelete() {
    if (!server || !onDelete) return;
    const confirmed = await confirm(
      `Delete server "${server.name}"? This will also delete all tunnels.`,
      { title: "Delete Server", kind: "warning" }
    );
    if (!confirmed) return;

    try {
      await invoke("delete_frp_server", { id: server.id });
      editing = false;
      await onDelete();
    } catch (e) {
      error = String(e);
    }
  }

  function goToServices() {
    if (onNavigateToServices) {
      onNavigateToServices();
    }
  }

  // Get status summary from polling
  let statusSummary = $derived(polling.getStatusSummary());
</script>

<section class="server-accordion" class:expanded>
  <button
    class="accordion-header"
    onclick={() => (expanded = !expanded)}
  >
    <div class="status-summary">
      <span class="status-dot {statusSummary.class}">{statusSummary.icon}</span>
      <div class="status-text">
        <span class="status-label">
          {#if server}
            {statusSummary.label} to {server.name}
          {:else}
            No server configured
          {/if}
        </span>
        <span class="status-detail">
          {#if server}
            {server.server_addr}:{server.server_port} | {statusSummary.detail}
          {:else}
            Set up an frp server to create tunnels
          {/if}
        </span>
      </div>
    </div>
    <div class="accordion-actions">
      <span class="accordion-arrow">{expanded ? "▲" : "▼"}</span>
    </div>
  </button>

  {#if expanded}
    <div class="accordion-content">
      {#if error}
        <div class="error-banner">
          {error}
          <button class="dismiss" onclick={() => (error = null)}>&times;</button>
        </div>
      {/if}

      {#if editing || !server}
        <div class="server-form">
          <div class="form-grid">
            <div class="form-row">
              <label>
                <span>Name</span>
                <input
                  type="text"
                  bind:value={serverName}
                  placeholder="My VPS"
                  disabled={saving}
                />
              </label>
            </div>
            <div class="form-row">
              <label>
                <span>Server Address</span>
                <div class="input-with-port">
                  <input
                    type="text"
                    bind:value={serverAddr}
                    placeholder="tunnel.example.com"
                    disabled={saving}
                  />
                  <span class="port-separator">:</span>
                  <input
                    type="number"
                    bind:value={serverPort}
                    class="port-input"
                    placeholder="7000"
                    disabled={saving}
                  />
                </div>
              </label>
            </div>
            <div class="form-row">
              <label>
                <span>Token</span>
                <div class="token-input">
                  <input
                    type="password"
                    bind:value={serverToken}
                    placeholder="Authentication token"
                    disabled={saving}
                  />
                  <button class="btn small" onclick={generateToken} disabled={saving}>
                    Generate
                  </button>
                </div>
              </label>
            </div>
            <div class="form-row">
              <label>
                <span>Subdomain Host</span>
                <input
                  type="text"
                  bind:value={subdomainHost}
                  placeholder="tunnel.example.com"
                  disabled={saving}
                />
              </label>
            </div>
          </div>
          <div class="form-actions">
            {#if server}
              <button class="btn danger small" onclick={handleDelete}>Delete Server</button>
            {/if}
            <div class="form-actions-right">
              {#if server}
                <button class="btn" onclick={cancelEdit}>Cancel</button>
              {/if}
              <button class="btn primary" onclick={save} disabled={saving}>
                {saving ? "Saving..." : (server ? "Save" : "Add Server")}
              </button>
            </div>
          </div>
        </div>
      {:else}
        <div class="server-display">
          <div class="server-info-grid">
            <div class="info-item">
              <span class="info-label">Name</span>
              <span class="info-value">{server.name}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Address</span>
              <span class="info-value mono">{server.server_addr}:{server.server_port}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Token</span>
              <span class="info-value mono">••••••••••••</span>
            </div>
            <div class="info-item">
              <span class="info-label">Subdomain Host</span>
              <span class="info-value mono">{server.subdomain_host}</span>
            </div>
          </div>
          <div class="server-display-actions">
            <button class="btn" onclick={loadForm}>Edit</button>
          </div>
        </div>
      {/if}

      <!-- Connection warning when not connected -->
      {#if frpcInstance && !polling.connectionStatus?.connected && statusSummary.class !== "connected"}
        <div class="connection-warning {statusSummary.class}">
          <span class="warning-icon">
            {#if statusSummary.class === "offline" || statusSummary.class === "stopped" || statusSummary.class === "error"}⚠{:else}⏳{/if}
          </span>
          <span class="warning-text">{statusSummary.detail}</span>
          {#if statusSummary.class === "stopped" && onStartFrpc}
            <button class="btn small primary" onclick={() => onStartFrpc?.(frpcInstance.id)}>Start</button>
          {:else if onNavigateToServices && (statusSummary.class === "offline" || statusSummary.class === "error")}
            <button class="btn small" onclick={goToServices}>Go to Services</button>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
</section>

<style>
  .server-accordion {
    background: white;
    border-radius: 12px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
    overflow: hidden;
  }

  @media (prefers-color-scheme: dark) {
    .server-accordion {
      background: #2c2c2e;
    }
  }

  .accordion-header {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.25rem;
    background: transparent;
    border: none;
    cursor: pointer;
    text-align: left;
    color: inherit;
  }

  .accordion-header:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  @media (prefers-color-scheme: dark) {
    .accordion-header:hover {
      background: rgba(255, 255, 255, 0.05);
    }
  }

  .status-summary {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .status-dot {
    font-size: 1rem;
    line-height: 1;
  }

  .status-dot.connected { color: #34c759; }
  .status-dot.connecting { color: #f59e0b; }
  .status-dot.checking { color: #3b82f6; }
  .status-dot.offline { color: #86868b; }
  .status-dot.stopped { color: #86868b; }
  .status-dot.error { color: #dc2626; }

  .status-text {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .status-label {
    font-weight: 500;
    font-size: 0.9375rem;
  }

  .status-detail {
    font-size: 0.8125rem;
    color: #86868b;
  }

  .accordion-actions {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .accordion-arrow {
    color: #86868b;
    font-size: 0.75rem;
  }

  .accordion-content {
    padding: 0 1.25rem 1.25rem;
    border-top: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .accordion-content {
      border-top-color: #48484a;
    }
  }

  .server-form {
    padding-top: 1rem;
  }

  .server-display {
    padding-top: 1rem;
  }

  .server-info-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 0.75rem;
    margin-bottom: 1rem;
  }

  .info-item {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .info-label {
    font-size: 0.6875rem;
    text-transform: uppercase;
    color: #86868b;
    font-weight: 500;
    letter-spacing: 0.025em;
  }

  .info-value {
    font-size: 0.875rem;
  }

  .info-value.mono {
    font-family: monospace;
    font-size: 0.8125rem;
  }

  .server-display-actions {
    display: flex;
    gap: 0.5rem;
  }

  .connection-warning {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border-radius: 6px;
    margin-top: 1rem;
    font-size: 0.8125rem;
  }

  .connection-warning.connecting {
    background: #dbeafe;
    color: #1e40af;
  }

  .connection-warning.offline,
  .connection-warning.stopped,
  .connection-warning.error {
    background: #fef3c7;
    color: #92400e;
  }

  @media (prefers-color-scheme: dark) {
    .connection-warning.connecting {
      background: #1e3a5f;
      color: #93c5fd;
    }

    .connection-warning.offline,
    .connection-warning.stopped,
    .connection-warning.error {
      background: #422006;
      color: #fbbf24;
    }
  }

  .warning-icon {
    font-size: 1rem;
  }

  .warning-text {
    flex: 1;
  }

  /* Form styles */
  .form-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 1rem;
  }

  .form-row {
    display: flex;
    flex-direction: column;
  }

  .form-row label {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .form-row label span {
    font-size: 0.6875rem;
    font-weight: 500;
    color: #86868b;
    text-transform: uppercase;
    letter-spacing: 0.025em;
  }

  .form-row input {
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
  }

  @media (prefers-color-scheme: dark) {
    .form-row input {
      background: #1c1c1e;
      border-color: #48484a;
      color: white;
    }
  }

  .input-with-port {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .input-with-port input:first-child {
    flex: 1;
  }

  .port-separator {
    color: #86868b;
  }

  .port-input {
    width: 70px;
  }

  .token-input {
    display: flex;
    gap: 0.5rem;
  }

  .token-input input {
    flex: 1;
  }

  .form-actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .form-actions {
      border-top-color: #48484a;
    }
  }

  .form-actions-right {
    display: flex;
    gap: 0.5rem;
  }

  /* Error banner */
  .error-banner {
    background: #fee2e2;
    color: #dc2626;
    padding: 0.75rem 1rem;
    border-radius: 6px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.875rem;
    margin-top: 1rem;
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

  /* Buttons */
  .btn {
    padding: 0.5rem 1rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    background: white;
  }

  @media (prefers-color-scheme: dark) {
    .btn {
      background: #2c2c2e;
      border-color: #48484a;
      color: white;
    }
  }

  .btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
  }

  .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
    border: none;
  }

  .btn.danger {
    background: transparent;
    color: #dc2626;
    border-color: #dc2626;
  }

  .btn.danger:hover {
    background: #dc2626;
    color: white;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Theme overrides */
  :global(:root[data-theme="light"]) .server-accordion {
    background: white !important;
  }

  :global(:root[data-theme="dark"]) .server-accordion {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="light"]) .accordion-content {
    border-top-color: #e5e5e7 !important;
  }

  :global(:root[data-theme="dark"]) .accordion-content {
    border-top-color: #48484a !important;
  }

  :global(:root[data-theme="light"]) .form-row input {
    background: white !important;
    border-color: #d1d1d6 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="dark"]) .form-row input {
    background: #1c1c1e !important;
    border-color: #48484a !important;
    color: white !important;
  }

  :global(:root[data-theme="light"]) .btn {
    background: white !important;
    border-color: #d1d1d6 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="dark"]) .btn {
    background: #2c2c2e !important;
    border-color: #48484a !important;
    color: white !important;
  }

  :global(:root[data-theme="light"]) .btn.primary,
  :global(:root[data-theme="dark"]) .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24) !important;
    color: white !important;
    border: none !important;
  }

  :global(:root[data-theme="light"]) .accordion-header:hover {
    background: rgba(0, 0, 0, 0.04) !important;
  }

  :global(:root[data-theme="dark"]) .accordion-header:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="light"]) .form-actions {
    border-top-color: #e5e5e7 !important;
  }

  :global(:root[data-theme="dark"]) .form-actions {
    border-top-color: #48484a !important;
  }
</style>
