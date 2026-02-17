<script lang="ts">
  import type { FrpcPolling } from "$lib/composables/useFrpcPolling.svelte";

  interface Tunnel {
    id: string;
    name: string;
    server_id: string;
    target: { type: "Instance" | "Port"; value: string | number };
    subdomain: { type: "Random" | "Custom"; generated?: string; subdomain?: string };
    protocol: string;
    auto_start: boolean;
    created_at: string;
  }

  interface TunnelState {
    running: boolean;
    public_url: string | null;
    error: string | null;
  }

  interface TunnelWithState {
    tunnel: Tunnel;
    state: TunnelState;
    server_name: string | null;
    target_name: string | null;
    target_port: number | null;
  }

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
    tunnels,
    server,
    frpcInstance,
    polling,
    loading = false,
    onEdit,
    onDelete,
    onViewConfig,
  }: {
    tunnels: TunnelWithState[];
    server: FrpServer | null;
    frpcInstance: FrpcInstance | undefined;
    polling: FrpcPolling;
    loading?: boolean;
    onEdit: (tunnel: TunnelWithState) => void;
    onDelete: (tunnel: TunnelWithState) => void;
    onViewConfig?: () => void;
  } = $props();

  let copiedId = $state<string | null>(null);

  function getSubdomain(tunnel: Tunnel): string {
    if (tunnel.subdomain.type === "Custom") {
      return tunnel.subdomain.subdomain || "";
    }
    return tunnel.subdomain.generated || "(not generated)";
  }

  function getExpectedUrl(tunnel: TunnelWithState): string | null {
    if (!server) return null;
    const subdomain = getSubdomain(tunnel.tunnel);
    if (!subdomain || subdomain === "(not generated)") return null;
    const protocol = tunnel.tunnel.protocol === "https" ? "https" : "http";
    return `${protocol}://${subdomain}.${server.subdomain_host}`;
  }

  function getTunnelStatus(t: TunnelWithState): { label: string; class: string } {
    const proxyStatus = polling.getProxyStatus(t.tunnel.id);
    const connectionStatus = polling.connectionStatus;

    if (!frpcInstance) {
      return { label: "Offline", class: "offline" };
    }
    if (!frpcInstance.running) {
      return { label: "Offline", class: "offline" };
    }
    if (connectionStatus === null) {
      return { label: "Checking...", class: "checking" };
    }
    if (!connectionStatus.running) {
      return { label: "Offline", class: "offline" };
    }
    if (!connectionStatus.connected) {
      return { label: "Connecting", class: "connecting" };
    }
    if (proxyStatus?.status === "running") {
      return { label: "Connected", class: "connected" };
    }
    if (proxyStatus?.status === "waiting") {
      return { label: "Connecting", class: "connecting" };
    }
    if (proxyStatus?.status === "error" || proxyStatus?.error) {
      return { label: "Error", class: "error" };
    }
    return { label: "Pending", class: "pending" };
  }

  function copyToClipboard(text: string, id?: string) {
    navigator.clipboard.writeText(text);
    if (id) {
      copiedId = id;
      setTimeout(() => {
        if (copiedId === id) copiedId = null;
      }, 1500);
    }
  }
</script>

<section class="card tunnels-card">
  <div class="card-header">
    <h3>Tunnels</h3>
    {#if server && onViewConfig}
      <button class="btn-link small" onclick={onViewConfig}>View Config</button>
    {/if}
  </div>

  {#if loading}
    <div class="loading">Loading tunnels...</div>
  {:else if tunnels.length === 0}
    <div class="empty-state">
      <p>No tunnels configured yet.</p>
      <p class="hint">
        {server === null
          ? "Add an frp server first, then create tunnels."
          : "Create a tunnel to expose a local service to the internet."}
      </p>
    </div>
  {:else}
    <div class="tunnels-table-wrapper">
      <table class="tunnels-table">
        <thead>
          <tr>
            <th class="col-name">Name</th>
            <th class="col-local">Local</th>
            <th class="col-url">URL</th>
            <th class="col-status">Status</th>
            <th class="col-actions"></th>
          </tr>
        </thead>
        <tbody>
          {#each tunnels as t}
            {@const expectedUrl = getExpectedUrl(t)}
            {@const status = getTunnelStatus(t)}
            {@const proxyStatus = polling.getProxyStatus(t.tunnel.id)}
            <tr class="tunnel-row">
              <td class="col-name">
                <span class="tunnel-name">{t.tunnel.name}</span>
              </td>
              <td class="col-local">
                <span class="local-target">
                  {#if t.target_name}
                    {t.target_name}
                  {:else}
                    :{t.target_port}
                  {/if}
                </span>
              </td>
              <td class="col-url">
                {#if t.state.running && t.state.public_url}
                  {@const url = t.state.public_url}
                  <div class="url-cell">
                    <a
                      href={url}
                      target="_blank"
                      rel="noopener noreferrer"
                      class="url-link"
                      title={url}
                    >
                      {url.replace(/^https?:\/\//, '').slice(0, 28)}{url.replace(/^https?:\/\//, '').length > 28 ? '...' : ''}
                    </a>
                    <button
                      class="copy-btn"
                      onclick={() => copyToClipboard(url, t.tunnel.id)}
                      title="Copy URL"
                    >
                      {copiedId === t.tunnel.id ? "✓" : "⧉"}
                    </button>
                  </div>
                {:else if expectedUrl}
                  <div class="url-cell">
                    <a
                      href={expectedUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      class="url-link expected"
                      title={expectedUrl}
                    >
                      {expectedUrl.replace(/^https?:\/\//, '').slice(0, 28)}{expectedUrl.replace(/^https?:\/\//, '').length > 28 ? '...' : ''}
                    </a>
                    <button
                      class="copy-btn"
                      onclick={() => copyToClipboard(expectedUrl, t.tunnel.id)}
                      title="Copy URL"
                    >
                      {copiedId === t.tunnel.id ? "✓" : "⧉"}
                    </button>
                  </div>
                {:else}
                  <span class="no-url">—</span>
                {/if}
              </td>
              <td class="col-status">
                <span class="status-badge {status.class}" title={proxyStatus?.error || ''}>
                  {status.label}
                </span>
              </td>
              <td class="col-actions">
                <div class="action-buttons">
                  <button class="icon-btn" onclick={() => onEdit(t)} title="Edit">
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                    </svg>
                  </button>
                  <button class="icon-btn danger" onclick={() => onDelete(t)} title="Delete">
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      <polyline points="3 6 5 6 21 6"></polyline>
                      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                    </svg>
                  </button>
                </div>
              </td>
            </tr>
            {#if proxyStatus?.error || t.state.error}
              <tr class="error-row">
                <td colspan="5">
                  <span class="error-text">{proxyStatus?.error || t.state.error}</span>
                </td>
              </tr>
            {/if}
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>

<style>
  .card {
    background: white;
    border-radius: 12px;
    padding: 1.25rem;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.05);
  }

  @media (prefers-color-scheme: dark) {
    .card {
      background: #2c2c2e;
    }
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.75rem;
  }

  .card-header h3 {
    margin: 0;
    font-size: 0.9375rem;
    font-weight: 600;
  }

  .tunnels-table-wrapper {
    overflow-x: auto;
  }

  .tunnels-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  .tunnels-table th {
    text-align: left;
    padding: 0.5rem 0.75rem;
    font-weight: 500;
    font-size: 0.75rem;
    text-transform: uppercase;
    color: #86868b;
    letter-spacing: 0.025em;
    border-bottom: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .tunnels-table th {
      border-bottom-color: #48484a;
    }
  }

  .tunnels-table td {
    padding: 0.625rem 0.75rem;
    border-bottom: 1px solid #f0f0f0;
    vertical-align: middle;
  }

  @media (prefers-color-scheme: dark) {
    .tunnels-table td {
      border-bottom-color: #38383a;
    }
  }

  .tunnel-row:last-child td {
    border-bottom: none;
  }

  .tunnel-row:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  @media (prefers-color-scheme: dark) {
    .tunnel-row:hover {
      background: rgba(255, 255, 255, 0.05);
    }
  }

  .col-name { width: 20%; }
  .col-local { width: 15%; }
  .col-url { width: 40%; }
  .col-status { width: 15%; }
  .col-actions { width: 10%; text-align: right; }

  .tunnel-name {
    font-weight: 500;
  }

  .local-target {
    font-family: monospace;
    color: #86868b;
  }

  .url-cell {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .url-link {
    color: #007aff;
    text-decoration: none;
    font-family: monospace;
    font-size: 0.8125rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 250px;
  }

  .url-link.expected {
    color: #86868b;
  }

  .url-link:hover {
    text-decoration: underline;
  }

  .copy-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.25rem;
    color: #86868b;
    font-size: 0.875rem;
    line-height: 1;
    border-radius: 3px;
  }

  .copy-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #007aff;
  }

  .no-url {
    color: #c7c7cc;
  }

  .status-badge {
    display: inline-block;
    padding: 0.1875rem 0.5rem;
    border-radius: 100px;
    font-size: 0.75rem;
    font-weight: 500;
  }

  .status-badge.connected {
    background: #dcfce7;
    color: #166534;
  }

  .status-badge.connecting,
  .status-badge.checking,
  .status-badge.pending {
    background: #fef3c7;
    color: #92400e;
  }

  .status-badge.offline {
    background: #f3f4f6;
    color: #6b7280;
  }

  .status-badge.error {
    background: #fee2e2;
    color: #dc2626;
  }

  @media (prefers-color-scheme: dark) {
    .status-badge.connected {
      background: #14532d;
      color: #86efac;
    }

    .status-badge.connecting,
    .status-badge.checking,
    .status-badge.pending {
      background: #422006;
      color: #fbbf24;
    }

    .status-badge.offline {
      background: #27272a;
      color: #a1a1aa;
    }

    .status-badge.error {
      background: #3d2020;
      color: #fca5a5;
    }
  }

  .action-buttons {
    display: flex;
    gap: 0.25rem;
    justify-content: flex-end;
  }

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

  .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1);
    color: #dc2626;
  }

  @media (prefers-color-scheme: dark) {
    .icon-btn {
      color: #98989d;
    }
    .icon-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
    .icon-btn.danger:hover {
      background: rgba(239, 68, 68, 0.2);
      color: #ef4444;
    }
  }

  .error-row td {
    padding: 0 0.75rem 0.5rem;
    border-bottom: none;
  }

  .error-text {
    font-size: 0.75rem;
    color: #dc2626;
    font-family: monospace;
  }

  .loading,
  .empty-state {
    text-align: center;
    padding: 2rem;
    color: #86868b;
  }

  .empty-state p {
    margin: 0 0 0.5rem;
  }

  .empty-state .hint {
    font-size: 0.875rem;
    opacity: 0.8;
  }

  .btn-link {
    background: none;
    border: none;
    color: #007aff;
    font-size: 0.875rem;
    cursor: pointer;
    padding: 0;
  }

  .btn-link.small {
    font-size: 0.8125rem;
  }

  .btn-link:hover {
    text-decoration: underline;
  }

  /* Theme overrides */
  :global(:root[data-theme="light"]) .card {
    background: white !important;
  }

  :global(:root[data-theme="dark"]) .card {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="light"]) .tunnels-table th {
    border-bottom-color: #e5e5e7 !important;
  }

  :global(:root[data-theme="dark"]) .tunnels-table th {
    border-bottom-color: #48484a !important;
  }

  :global(:root[data-theme="light"]) .tunnels-table td {
    border-bottom-color: #f0f0f2 !important;
  }

  :global(:root[data-theme="dark"]) .tunnels-table td {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="light"]) .tunnel-row:hover {
    background: rgba(0, 0, 0, 0.04) !important;
  }

  :global(:root[data-theme="dark"]) .tunnel-row:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="light"]) .icon-btn {
    color: #86868b !important;
  }

  :global(:root[data-theme="light"]) .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn {
    color: #98989d !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }
</style>
