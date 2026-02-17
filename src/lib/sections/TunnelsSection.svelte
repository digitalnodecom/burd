<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { createFrpcPolling } from "$lib/composables/useFrpcPolling.svelte";
  import FrpcServerManager from "$lib/components/FrpcServerManager.svelte";
  import TunnelList from "$lib/components/TunnelList.svelte";

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

  interface TunnelTarget {
    type: "Instance" | "Port";
    value: string | number;
  }

  interface SubdomainConfig {
    type: "Random" | "Custom";
    generated?: string;
    subdomain?: string;
  }

  interface Tunnel {
    id: string;
    name: string;
    server_id: string;
    target: TunnelTarget;
    subdomain: SubdomainConfig;
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

  interface Instance {
    id: string;
    name: string;
    port: number;
    service_type: string;
    running: boolean;
    healthy: boolean | null;
  }

  interface VersionInfo {
    version: string;
    is_latest: boolean;
  }

  let {
    instances = [],
    onNavigateToServices,
    onNavigateToInstances,
    onTunnelCountChange,
    onRefresh,
    onStartFrpc,
  }: {
    instances: Instance[];
    onNavigateToServices?: () => void;
    onNavigateToInstances?: () => void;
    onTunnelCountChange?: (count: number) => void;
    onRefresh?: () => void;
    onStartFrpc?: (id: string) => void;
  } = $props();

  // Derived: find the frpc instance
  let frpcInstance = $derived(instances.find(i => i.service_type === "Tunnels (frpc)"));

  // State
  let servers = $state<FrpServer[]>([]);
  let tunnels = $state<TunnelWithState[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  // frpc binary state
  let frpcInstalled = $state(false);
  let frpcVersions = $state<string[]>([]);
  let selectedFrpcVersion = $state("");
  let downloadingFrpc = $state(false);

  // Server accordion state
  let serverExpanded = $state(false);

  // Tunnel form state
  let showTunnelForm = $state(false);
  let editingTunnel = $state<TunnelWithState | null>(null);
  let tunnelName = $state("");
  let tunnelServerId = $state("");
  let tunnelTargetType = $state<"instance" | "port">("port");
  let tunnelTargetValue = $state("");
  let tunnelSubdomainType = $state<"random" | "custom">("random");
  let tunnelSubdomainValue = $state("");
  let tunnelProtocol = $state<"http" | "https">("http");
  let savingTunnel = $state(false);

  // View config modal state
  let showConfigModal = $state(false);
  let frpcConfig = $state("");

  // Copy feedback state
  let copiedId = $state<string | null>(null);

  // Derived: the single server (we only support one)
  let server = $derived(servers.length > 0 ? servers[0] : null);

  // Create polling composable
  const polling = createFrpcPolling(
    () => frpcInstance,
    () => tunnels.length
  );

  // Report tunnel count changes
  $effect(() => {
    const count = polling.getConnectedTunnelCount();
    if (onTunnelCountChange) {
      onTunnelCountChange(count);
    }
  });

  // Watch for frpc instance changes to start/stop polling
  $effect(() => {
    if (frpcInstance) {
      polling.startPolling();
    } else {
      polling.stopPolling();
    }
    return () => polling.cleanup();
  });

  async function loadData() {
    loading = true;
    error = null;
    try {
      const [serverList, tunnelList, isInstalled] = await Promise.all([
        invoke<FrpServer[]>("list_frp_servers"),
        invoke<TunnelWithState[]>("list_tunnels"),
        invoke<boolean>("check_frpc_installed"),
      ]);
      servers = serverList;
      tunnels = tunnelList;
      frpcInstalled = isInstalled;

      // Load available versions if frpc is not installed
      if (!frpcInstalled) {
        try {
          const versionInfos = await invoke<VersionInfo[]>("get_available_versions", { serviceType: "frpc" });
          frpcVersions = versionInfos.map(v => v.version);
          if (frpcVersions.length > 0) {
            selectedFrpcVersion = frpcVersions[0];
          }
        } catch (e) {
          console.error("Failed to load frpc versions:", e);
        }
      }

      // Set default server for new tunnels
      if (servers.length > 0 && !tunnelServerId) {
        const defaultServer = servers.find(s => s.is_default) || servers[0];
        tunnelServerId = defaultServer.id;
      }

      // Auto-expand server section if no server configured
      if (servers.length === 0) {
        serverExpanded = true;
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function downloadFrpc() {
    if (!selectedFrpcVersion) return;

    downloadingFrpc = true;
    error = null;
    try {
      await invoke("download_binary", {
        serviceType: "frpc",
        version: selectedFrpcVersion,
      });
      frpcInstalled = true;
    } catch (e) {
      error = String(e);
    } finally {
      downloadingFrpc = false;
    }
  }

  function resetTunnelForm() {
    editingTunnel = null;
    tunnelName = "";
    tunnelTargetType = "port";
    tunnelTargetValue = "";
    tunnelSubdomainType = "random";
    tunnelSubdomainValue = "";
    tunnelProtocol = "http";
    showTunnelForm = false;
  }

  function editTunnel(t: TunnelWithState) {
    editingTunnel = t;
    tunnelName = t.tunnel.name;
    tunnelServerId = t.tunnel.server_id;

    if (t.tunnel.target.type === "Instance") {
      tunnelTargetType = "instance";
      tunnelTargetValue = String(t.tunnel.target.value);
    } else {
      tunnelTargetType = "port";
      tunnelTargetValue = String(t.target_port || t.tunnel.target.value);
    }

    if (t.tunnel.subdomain.type === "Custom") {
      tunnelSubdomainType = "custom";
      tunnelSubdomainValue = t.tunnel.subdomain.subdomain || "";
    } else {
      tunnelSubdomainType = "random";
      tunnelSubdomainValue = "";
    }

    tunnelProtocol = t.tunnel.protocol === "https" ? "https" : "http";
    showTunnelForm = true;
  }

  async function saveTunnel() {
    if (!tunnelName.trim()) {
      error = "Tunnel name is required";
      return;
    }
    if (!tunnelServerId) {
      error = "Please select a server";
      return;
    }
    if (!tunnelTargetValue) {
      error = "Target is required";
      return;
    }
    if (tunnelSubdomainType === "custom" && !tunnelSubdomainValue.trim()) {
      error = "Custom subdomain is required";
      return;
    }

    savingTunnel = true;
    error = null;
    try {
      if (editingTunnel) {
        await invoke("update_tunnel", {
          id: editingTunnel.tunnel.id,
          name: tunnelName.trim(),
          server_id: tunnelServerId,
          target_type: tunnelTargetType,
          target_value: String(tunnelTargetValue),
          subdomain_type: tunnelSubdomainType,
          subdomain_value: tunnelSubdomainType === "custom" ? tunnelSubdomainValue.trim() : null,
          protocol: tunnelProtocol,
        });
      } else {
        await invoke("create_tunnel", {
          request: {
            name: tunnelName.trim(),
            server_id: tunnelServerId,
            target_type: tunnelTargetType,
            target_value: String(tunnelTargetValue),
            subdomain_type: tunnelSubdomainType,
            subdomain_value: tunnelSubdomainType === "custom" ? tunnelSubdomainValue.trim() : null,
            protocol: tunnelProtocol,
          },
        });
      }
      resetTunnelForm();
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      savingTunnel = false;
    }
  }

  async function deleteTunnel(tunnel: TunnelWithState) {
    const confirmed = await confirm(
      `Delete tunnel "${tunnel.tunnel.name}"?`,
      { title: "Delete Tunnel", kind: "warning" }
    );
    if (!confirmed) return;

    try {
      await invoke("delete_tunnel", { id: tunnel.tunnel.id });
      await loadData();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadFrpcConfig() {
    try {
      frpcConfig = await invoke<string>("get_frpc_config");
      showConfigModal = true;
    } catch (e) {
      error = String(e);
    }
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

  onMount(() => {
    loadData();
  });
</script>

<div class="tunnels-section">
  <div class="section-header">
    <div class="title-row">
      <h2>Tunnels</h2>
      {#if onRefresh}
        <button class="refresh-btn" onclick={onRefresh} title="Refresh">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 2v6h-6"></path>
            <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
            <path d="M3 22v-6h6"></path>
            <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
          </svg>
        </button>
      {/if}
    </div>
    <div class="header-actions">
      <button
        class="btn secondary small"
        onclick={() => (showTunnelForm = !showTunnelForm)}
        disabled={servers.length === 0}
      >
        {showTunnelForm ? "Cancel" : "+ New Tunnel"}
      </button>
    </div>
  </div>

  {#if error}
    <div class="error-banner">
      {error}
      <button class="dismiss" onclick={() => (error = null)}>&times;</button>
    </div>
  {/if}

  <!-- frpc Binary Status -->
  {#if !frpcInstalled}
    <section class="card warning">
      <div class="card-header">
        <h3>frpc Binary Required</h3>
      </div>
      <div class="binary-install">
        <p>The frp client (frpc) binary is required to create tunnels. Download it to get started.</p>
        <div class="install-controls">
          {#if frpcVersions.length > 0}
            <select bind:value={selectedFrpcVersion} disabled={downloadingFrpc}>
              {#each frpcVersions as version}
                <option value={version}>{version}</option>
              {/each}
            </select>
            <button class="btn primary" onclick={downloadFrpc} disabled={downloadingFrpc}>
              {downloadingFrpc ? "Downloading..." : "Download frpc"}
            </button>
          {:else}
            <span class="loading-text">Loading versions...</span>
          {/if}
        </div>
      </div>
    </section>
  {:else if !frpcInstance}
    <section class="card warning">
      <div class="card-header">
        <h3>frpc Instance Required</h3>
      </div>
      <div class="binary-install">
        <p>Create a Tunnels (frpc) instance to manage tunnels. Go to the Instances page to create one.</p>
        <div class="install-controls">
          <button class="btn primary" onclick={() => onNavigateToInstances?.()}>
            Go to Instances
          </button>
        </div>
      </div>
    </section>
  {:else if !frpcInstance.running}
    <section class="card info">
      <div class="card-header">
        <h3>frpc Not Running</h3>
      </div>
      <div class="binary-install">
        <p>The Tunnels (frpc) instance is installed but not running. Start it to manage tunnels.</p>
        <div class="install-controls">
          <button class="btn primary" onclick={() => onStartFrpc?.(frpcInstance.id)}>
            Start frpc
          </button>
        </div>
      </div>
    </section>
  {/if}

  <!-- Server + Status Accordion -->
  <FrpcServerManager
    {server}
    {frpcInstance}
    {polling}
    bind:expanded={serverExpanded}
    onSave={loadData}
    onDelete={loadData}
    {onStartFrpc}
    {onNavigateToServices}
  />

  <!-- Tunnel Form -->
  {#if showTunnelForm}
    <section class="card tunnel-form-card">
      <div class="card-header">
        <h3>{editingTunnel ? "Edit Tunnel" : "Create New Tunnel"}</h3>
      </div>
      <div class="form-grid">
        <div class="form-row">
          <label>
            <span>Tunnel Name</span>
            <input
              type="text"
              bind:value={tunnelName}
              placeholder="My API"
              disabled={savingTunnel}
            />
          </label>
        </div>
        <div class="form-row">
          <label>
            <span>Server</span>
            <select bind:value={tunnelServerId} disabled={savingTunnel}>
              {#each servers as s}
                <option value={s.id}>{s.name}</option>
              {/each}
            </select>
          </label>
        </div>
        <div class="form-row">
          <label>
            <span>Target Type</span>
            <select bind:value={tunnelTargetType} disabled={savingTunnel}>
              <option value="port">Port</option>
              <option value="instance">Instance</option>
            </select>
          </label>
        </div>
        <div class="form-row">
          <label>
            <span>Target {tunnelTargetType === "instance" ? "Instance" : "Port"}</span>
            {#if tunnelTargetType === "instance"}
              <select bind:value={tunnelTargetValue} disabled={savingTunnel}>
                <option value="">Select an instance...</option>
                {#each instances as inst}
                  <option value={inst.id}>{inst.name} (port {inst.port})</option>
                {/each}
              </select>
            {:else}
              <input
                type="number"
                bind:value={tunnelTargetValue}
                placeholder="8080"
                min="1"
                max="65535"
                disabled={savingTunnel}
              />
            {/if}
          </label>
        </div>
        <div class="form-row">
          <label>
            <span>Subdomain</span>
            <select bind:value={tunnelSubdomainType} disabled={savingTunnel}>
              <option value="random">Random</option>
              <option value="custom">Custom</option>
            </select>
          </label>
        </div>
        {#if tunnelSubdomainType === "custom"}
          <div class="form-row">
            <label>
              <span>Custom Subdomain</span>
              <input
                type="text"
                bind:value={tunnelSubdomainValue}
                placeholder="my-api"
                disabled={savingTunnel}
              />
            </label>
          </div>
        {/if}
        <div class="form-row">
          <label>
            <span>Protocol</span>
            <select bind:value={tunnelProtocol} disabled={savingTunnel}>
              <option value="http">HTTP</option>
              <option value="https">HTTPS</option>
            </select>
          </label>
        </div>
      </div>
      <div class="form-actions">
        <button class="btn" onclick={resetTunnelForm}>Cancel</button>
        <button class="btn primary" onclick={saveTunnel} disabled={savingTunnel}>
          {savingTunnel ? "Saving..." : (editingTunnel ? "Update Tunnel" : "Create Tunnel")}
        </button>
      </div>
    </section>
  {/if}

  <!-- Tunnels Table -->
  <TunnelList
    {tunnels}
    {server}
    {frpcInstance}
    {polling}
    {loading}
    onEdit={editTunnel}
    onDelete={deleteTunnel}
    onViewConfig={loadFrpcConfig}
  />
</div>

<!-- View Config Modal -->
{#if showConfigModal}
  <div class="modal-overlay" onclick={() => (showConfigModal = false)} onkeydown={(e) => e.key === 'Escape' && (showConfigModal = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal wide" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>frpc Configuration</h3>
        <button class="modal-close" onclick={() => (showConfigModal = false)}>&times;</button>
      </div>
      <div class="modal-content">
        <div class="code-block config-block">
          <pre><code>{frpcConfig}</code></pre>
          <button class="copy-code-btn" onclick={() => copyToClipboard(frpcConfig, 'frpc-config')}>
            {copiedId === 'frpc-config' ? 'Copied!' : 'Copy All'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .tunnels-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
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

  .refresh-btn:active {
    transform: rotate(180deg);
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

  .header-actions {
    display: flex;
    gap: 0.5rem;
  }

  .card {
    background: white;
    border-radius: 12px;
    padding: 1.25rem;
    border: 1px solid #e5e5e5;
  }

  .card.warning {
    background: #fef3c7;
    border: 1px solid #f59e0b;
  }

  .card.info {
    background: #dbeafe;
    border: 1px solid #3b82f6;
  }

  @media (prefers-color-scheme: dark) {
    .card.warning {
      background: #422006;
      border-color: #b45309;
    }

    .card.info {
      background: #1e3a5f;
      border-color: #2563eb;
    }
  }

  .card-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
  }

  .card-header h3 {
    margin: 0;
    font-size: 0.9375rem;
    font-weight: 600;
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

  .form-row input,
  .form-row select {
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
  }

  @media (prefers-color-scheme: dark) {
    .form-row input,
    .form-row select {
      background: #1c1c1e;
      border-color: #48484a;
      color: white;
    }
  }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
    padding-top: 1rem;
    border-top: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .form-actions {
      border-top-color: #48484a;
    }
  }

  /* Binary install */
  .binary-install {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .binary-install p {
    margin: 0;
    font-size: 0.875rem;
    color: #92400e;
  }

  @media (prefers-color-scheme: dark) {
    .binary-install p {
      color: #fbbf24;
    }
  }

  .install-controls {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  .install-controls select {
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
    min-width: 150px;
  }

  @media (prefers-color-scheme: dark) {
    .install-controls select {
      background: #2c2c2e;
      border-color: #48484a;
      color: white;
    }
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

  .loading-text {
    font-size: 0.875rem;
    color: #86868b;
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

  .btn.secondary {
    background: #007aff;
    color: white;
    border: none;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
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
  }

  .modal {
    background: white;
    border-radius: 12px;
    max-width: 500px;
    width: 90%;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }

  .modal.wide {
    max-width: 700px;
  }

  @media (prefers-color-scheme: dark) {
    .modal {
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

  .modal-close {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: #86868b;
    line-height: 1;
  }

  .modal-content {
    padding: 1.5rem;
    flex: 1;
    overflow-y: auto;
    min-height: 0;
  }

  .code-block {
    position: relative;
  }

  .code-block pre {
    background: #1c1c1e;
    color: #e5e5e5;
    padding: 1rem;
    border-radius: 8px;
    overflow-x: auto;
    font-size: 0.8125rem;
    line-height: 1.5;
    margin: 0;
  }

  .code-block pre code {
    background: none;
    padding: 0;
    color: inherit;
  }

  .copy-code-btn {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    background: rgba(255, 255, 255, 0.1);
    border: none;
    color: #a1a1aa;
    padding: 0.375rem 0.625rem;
    border-radius: 4px;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .copy-code-btn:hover {
    background: rgba(255, 255, 255, 0.2);
    color: white;
  }

  /* Theme overrides */
  :global(:root[data-theme="light"]) .card {
    background: white !important;
  }

  :global(:root[data-theme="light"]) .card.warning {
    background: #fef3c7 !important;
    border-color: #f59e0b !important;
  }

  :global(:root[data-theme="light"]) .card.info {
    background: #dbeafe !important;
    border-color: #3b82f6 !important;
  }

  :global(:root[data-theme="light"]) .form-row input,
  :global(:root[data-theme="light"]) .form-row select {
    background: white !important;
    border-color: #d1d1d6 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .btn {
    background: white !important;
    border-color: #d1d1d6 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24) !important;
    color: white !important;
  }

  :global(:root[data-theme="light"]) .btn.secondary {
    background: #007aff !important;
    color: white !important;
  }

  :global(:root[data-theme="light"]) .modal {
    background: white !important;
  }

  :global(:root[data-theme="light"]) .refresh-btn {
    color: #86868b !important;
  }

  :global(:root[data-theme="light"]) .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .binary-install p {
    color: #92400e !important;
  }

  :global(:root[data-theme="light"]) .install-controls select {
    background: white !important;
    border-color: #d1d1d6 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="dark"]) .card {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .card.warning {
    background: #451a03 !important;
    border-color: #b45309 !important;
  }

  :global(:root[data-theme="dark"]) .card.info {
    background: #1e3a5f !important;
    border-color: #2563eb !important;
  }

  :global(:root[data-theme="dark"]) .form-row input,
  :global(:root[data-theme="dark"]) .form-row select {
    background: #1c1c1e !important;
    border-color: #48484a !important;
    color: white !important;
  }

  :global(:root[data-theme="dark"]) .btn {
    background: #2c2c2e !important;
    border-color: #48484a !important;
    color: white !important;
  }

  :global(:root[data-theme="dark"]) .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24) !important;
    color: white !important;
    border: none !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #007aff !important;
    color: white !important;
    border: none !important;
  }

  :global(:root[data-theme="dark"]) .modal {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .modal-header {
    border-bottom-color: #48484a !important;
  }

  :global(:root[data-theme="dark"]) .error-banner {
    background: #3d2020 !important;
    color: #fca5a5 !important;
  }

  :global(:root[data-theme="dark"]) .form-actions {
    border-top-color: #48484a !important;
  }

  :global(:root[data-theme="dark"]) .binary-install p {
    color: #fcd34d !important;
  }

  :global(:root[data-theme="dark"]) .install-controls select {
    background: #2c2c2e !important;
    border-color: #48484a !important;
    color: white !important;
  }
</style>
