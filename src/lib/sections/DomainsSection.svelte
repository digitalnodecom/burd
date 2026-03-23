<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { confirm } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  interface DomainInfo {
    id: string;
    subdomain: string;
    full_domain: string;
    target_type: string;           // "instance", "port", or "static"
    target_value: string;
    target_name: string | null;
    target_port: number | null;
    static_path: string | null;    // Path for static file server
    static_browse: boolean | null; // Directory listing enabled
    ssl_enabled: boolean;
    created_at: string;
  }

  interface Instance {
    id: string;
    name: string;
    port: number;
    running: boolean;
    pid: number | null;
    healthy: boolean | null;
  }

  let {
    instances = [],
    tld = "burd",
    onRefresh = () => {},
  }: {
    instances: Instance[];
    tld: string;
    onRefresh?: () => void;
  } = $props();

  let domains = $state<DomainInfo[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  // New domain form state
  let showNewDomainForm = $state(false);
  let newSubdomain = $state("");
  let newTargetType = $state<"instance" | "port" | "static">("instance");
  let newTargetValue = $state("");
  let newStaticPath = $state("");
  let newStaticBrowse = $state(true);
  let newSslEnabled = $state(true);
  let creating = $state(false);

  // Edit domain state
  let editingDomain = $state<DomainInfo | null>(null);

  // View Config modal state
  interface ProxyConfigInfo {
    routes_file: string;
    routes: Record<string, unknown>;
    daemon_installed: boolean;
    daemon_running: boolean;
    tld: string;
    certs_dir: string;
  }
  let showConfigModal = $state(false);
  let proxyConfig = $state<ProxyConfigInfo | null>(null);
  let loadingConfig = $state(false);

  // Drag-and-drop reordering state
  let isDragging = $state(false);
  let draggedDomainId = $state<string | null>(null);
  let dragOverIndex = $state<number | null>(null);
  let draggedFromIndex = $state<number | null>(null);

  // Mouse-based drag-and-drop for domain reordering
  $effect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      const el = document.elementFromPoint(e.clientX, e.clientY);
      // Walk up to find the <tr> with data-domain-id
      const row = el?.closest?.('tr[data-domain-id]') as HTMLElement | null;

      if (row) {
        const indexStr = row.getAttribute('data-index');
        const domainId = row.getAttribute('data-domain-id');
        if (indexStr && domainId !== draggedDomainId) {
          dragOverIndex = parseInt(indexStr, 10);
        }
      } else {
        dragOverIndex = null;
      }
    };

    const handleMouseUp = async () => {
      if (draggedFromIndex !== null && dragOverIndex !== null && draggedFromIndex !== dragOverIndex) {
        try {
          const newOrder = [...domains];
          const [dragged] = newOrder.splice(draggedFromIndex, 1);
          newOrder.splice(dragOverIndex, 0, dragged);
          const orderedIds = newOrder.map(d => d.id);
          await invoke('reorder_domains', { domainIds: orderedIds });
          await loadDomains();
        } catch (e) {
          error = `Failed to reorder domains: ${e}`;
        }
      }

      document.body.classList.remove('dragging-domain');
      isDragging = false;
      draggedDomainId = null;
      dragOverIndex = null;
      draggedFromIndex = null;
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  });

  function handleDomainDragStart(event: MouseEvent, domainId: string, index: number) {
    event.preventDefault();
    isDragging = true;
    draggedDomainId = domainId;
    draggedFromIndex = index;
    document.body.classList.add('dragging-domain');
  }

  async function loadDomains() {
    console.log("[DomainsSection] loadDomains called");
    loading = true;
    error = null;
    try {
      console.log("[DomainsSection] invoking list_domains...");
      const result = await invoke<DomainInfo[]>("list_domains");
      console.log("[DomainsSection] list_domains returned:", result);
      domains = result;
    } catch (e) {
      console.error("[DomainsSection] list_domains error:", e);
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function createDomain() {
    if (!newSubdomain.trim()) {
      error = "Subdomain is required";
      return;
    }
    if (newTargetType === "static") {
      if (!newStaticPath.trim()) {
        error = "Directory path is required";
        return;
      }
    } else if (!newTargetValue) {
      error = "Target is required";
      return;
    }

    creating = true;
    error = null;
    try {
      let request: Record<string, unknown>;

      if (newTargetType === "static") {
        request = {
          subdomain: newSubdomain.trim().toLowerCase(),
          target_type: newTargetType,
          path: newStaticPath.trim(),
          browse: newStaticBrowse,
          ssl_enabled: newSslEnabled,
        };
      } else {
        request = {
          subdomain: newSubdomain.trim().toLowerCase(),
          target_type: newTargetType,
          target_value: newTargetType === "port" ? Number(newTargetValue) : newTargetValue,
          ssl_enabled: newSslEnabled,
        };
      }

      await invoke("create_domain", { request });
      newSubdomain = "";
      newTargetType = "instance";
      newTargetValue = "";
      newStaticPath = "";
      newStaticBrowse = true;
      newSslEnabled = true;
      showNewDomainForm = false;
      await loadDomains();
      onRefresh();
    } catch (e) {
      error = String(e);
    } finally {
      creating = false;
    }
  }

  async function deleteDomain(domain: DomainInfo) {
    const confirmed = await confirm(
      `Are you sure you want to delete ${domain.full_domain}?`,
      { title: "Delete Domain", kind: "warning" }
    );
    if (!confirmed) return;

    error = null;
    try {
      await invoke("delete_domain", { id: domain.id });
      await loadDomains();
      onRefresh();
    } catch (e) {
      error = String(e);
    }
  }

  function startEditDomain(domain: DomainInfo) {
    editingDomain = domain;
    newSubdomain = domain.subdomain;

    if (domain.target_type === "static") {
      newTargetType = "static";
      newTargetValue = "";
      newStaticPath = domain.static_path || "";
      newStaticBrowse = domain.static_browse ?? true;
    } else if (domain.target_type === "instance") {
      newTargetType = "instance";
      newTargetValue = domain.target_value || "";
      newStaticPath = "";
      newStaticBrowse = true;
    } else {
      newTargetType = "port";
      newTargetValue = String(domain.target_port || domain.target_value);
      newStaticPath = "";
      newStaticBrowse = true;
    }

    newSslEnabled = domain.ssl_enabled;
    showNewDomainForm = true;
  }

  async function updateDomain() {
    if (!editingDomain) return;
    if (!newSubdomain.trim()) {
      error = "Subdomain is required";
      return;
    }
    if (newTargetType === "static") {
      if (!newStaticPath.trim()) {
        error = "Directory path is required";
        return;
      }
    } else if (!newTargetValue) {
      error = "Target is required";
      return;
    }

    creating = true;
    error = null;
    try {
      // Delete old and create new (since we don't have an update endpoint)
      await invoke("delete_domain", { id: editingDomain.id });

      let request: Record<string, unknown>;
      if (newTargetType === "static") {
        request = {
          subdomain: newSubdomain.trim().toLowerCase(),
          target_type: newTargetType,
          path: newStaticPath.trim(),
          browse: newStaticBrowse,
          ssl_enabled: newSslEnabled,
        };
      } else {
        request = {
          subdomain: newSubdomain.trim().toLowerCase(),
          target_type: newTargetType,
          target_value: newTargetType === "port" ? Number(newTargetValue) : newTargetValue,
          ssl_enabled: newSslEnabled,
        };
      }

      await invoke("create_domain", { request });
      cancelEdit();
      await loadDomains();
      onRefresh();
    } catch (e) {
      error = String(e);
    } finally {
      creating = false;
    }
  }

  function cancelEdit() {
    editingDomain = null;
    newSubdomain = "";
    newTargetType = "instance";
    newTargetValue = "";
    newStaticPath = "";
    newStaticBrowse = true;
    newSslEnabled = true;
    showNewDomainForm = false;
  }

  async function toggleDomainSsl(domain: DomainInfo) {
    try {
      await invoke("update_domain_ssl", {
        id: domain.id,
        sslEnabled: !domain.ssl_enabled
      });
      await loadDomains();
    } catch (e) {
      error = `Failed to toggle SSL: ${e}`;
    }
  }

  async function loadProxyConfig() {
    loadingConfig = true;
    error = null;
    try {
      proxyConfig = await invoke<ProxyConfigInfo>("get_proxy_config");
      showConfigModal = true;
    } catch (e) {
      error = `Failed to load proxy config: ${String(e)}`;
    } finally {
      loadingConfig = false;
    }
  }

  // Copy to clipboard
  let copiedId = $state<string | null>(null);
  async function copyToClipboard(text: string, id: string) {
    await navigator.clipboard.writeText(text);
    copiedId = id;
    setTimeout(() => {
      if (copiedId === id) copiedId = null;
    }, 2000);
  }

  // Get instance info for a domain targeting an instance
  function getInstanceForDomain(domain: DomainInfo): Instance | undefined {
    if (domain.target_type === "instance") {
      return instances.find(i => i.id === domain.target_value);
    }
    return undefined;
  }

  // Port status cache: port -> active boolean
  let portStatuses = $state<Record<number, boolean>>({});

  // Check port statuses for all port-type domains
  async function checkPortStatuses() {
    const portDomains = domains.filter(d => d.target_type === "port");
    for (const d of portDomains) {
      const port = d.target_port ?? parseInt(d.target_value);
      if (port > 0) {
        try {
          const active = await invoke<boolean>("check_port_status", { port });
          portStatuses = { ...portStatuses, [port]: active };
        } catch {
          portStatuses = { ...portStatuses, [port]: false };
        }
      }
    }
  }

  function isPortActive(domain: DomainInfo): boolean {
    if (domain.target_type === "port") {
      const port = domain.target_port ?? parseInt(domain.target_value);
      return portStatuses[port] ?? false;
    }
    return false;
  }

  onMount(() => {
    loadDomains().then(() => checkPortStatuses());
    const interval = setInterval(checkPortStatuses, 10000);
    return () => clearInterval(interval);
  });
</script>

<div class="domains-section">
  <div class="section-header">
    <div class="title-row">
      <h2>Domains</h2>
      <button class="refresh-btn" onclick={() => loadDomains()} title="Refresh">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 2v6h-6"></path>
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
          <path d="M3 22v-6h6"></path>
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
        </svg>
      </button>
    </div>
    <div class="header-actions">
      <button
        class="btn secondary small"
        onclick={loadProxyConfig}
        disabled={loadingConfig}
        title="View proxy routes configuration"
      >
        {loadingConfig ? "Loading..." : "View Config"}
      </button>
      <button
        class="btn primary small"
        onclick={() => {
          if (showNewDomainForm) {
            cancelEdit();
          } else {
            showNewDomainForm = true;
          }
        }}
      >
        {showNewDomainForm ? "Cancel" : "+ New Domain"}
      </button>
    </div>
  </div>

  <section class="card">
    {#if error}
      <div class="error-banner">
        {error}
        <button class="dismiss" onclick={() => (error = null)}>&times;</button>
      </div>
    {/if}

    {#if showNewDomainForm}
      <div class="new-domain-form">
        <h3>{editingDomain ? "Edit Domain" : "Create New Domain"}</h3>
        <div class="form-row">
          <label>
            <span>Subdomain</span>
            <div class="subdomain-input">
              <input
                type="text"
                bind:value={newSubdomain}
                placeholder="my-api"
                disabled={creating}
              />
              <span class="tld-suffix">.{tld}</span>
            </div>
          </label>
        </div>

        <div class="form-row">
          <label>
            <span>Target Type</span>
            <select bind:value={newTargetType} disabled={creating}>
              <option value="instance">Instance</option>
              <option value="port">Port</option>
              <option value="static">Static Files</option>
            </select>
          </label>
        </div>

        {#if newTargetType === "instance"}
          <div class="form-row">
            <label>
              <span>Target Instance</span>
              <select bind:value={newTargetValue} disabled={creating}>
                <option value="">Select an instance...</option>
                {#each instances as inst}
                  <option value={inst.id}>{inst.name} (port {inst.port})</option>
                {/each}
              </select>
            </label>
          </div>
        {:else if newTargetType === "port"}
          <div class="form-row">
            <label>
              <span>Target Port</span>
              <input
                type="number"
                bind:value={newTargetValue}
                placeholder="8080"
                min="1"
                max="65535"
                disabled={creating}
              />
            </label>
          </div>
        {:else if newTargetType === "static"}
          <div class="form-row">
            <label>
              <span>Directory Path</span>
              <input
                type="text"
                bind:value={newStaticPath}
                placeholder="/var/www/html"
                disabled={creating}
              />
            </label>
          </div>
          <div class="form-row">
            <label class="checkbox-label">
              <input type="checkbox" bind:checked={newStaticBrowse} disabled={creating} />
              <span>Enable directory listing</span>
            </label>
          </div>
        {/if}

        <div class="form-row">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={newSslEnabled} disabled={creating} />
            <span>Enable SSL</span>
          </label>
        </div>

        <div class="form-actions">
          <button
            class="btn primary"
            onclick={editingDomain ? updateDomain : createDomain}
            disabled={creating}
          >
            {#if creating}
              {editingDomain ? "Updating..." : "Creating..."}
            {:else}
              {editingDomain ? "Update Domain" : "Create Domain"}
            {/if}
          </button>
        </div>
      </div>
    {/if}

    {#if loading}
      <div class="loading">Loading domains...</div>
    {:else if domains.length === 0}
      <div class="empty-state">
        <p>No domain mappings yet.</p>
        <p class="hint">
          Create a domain to route traffic from a custom subdomain to your services.
        </p>
      </div>
    {:else}
      <table class="domains-table">
        <thead>
          <tr>
            <th></th>
            <th>Domain</th>
            <th>Target</th>
            <th>Actions</th>
          </tr>
        </thead>
        <tbody>
          {#each domains as domain, i}
            {@const inst = getInstanceForDomain(domain)}
            {@const portActive = isPortActive(domain)}
            <tr data-domain-id={domain.id} data-index={i} class:dragging={draggedDomainId === domain.id} class:drag-over={dragOverIndex === i && draggedDomainId !== domain.id}>
              <td class="drag-handle" onmousedown={(e) => handleDomainDragStart(e, domain.id, i)}>&#x2807;</td>
              <td class="domain-cell">
                <div class="domain-row">
                  <a class="url-link" href="{domain.ssl_enabled ? 'https' : 'http'}://{domain.full_domain}" target="_blank" rel="noopener noreferrer">
                    {domain.full_domain}
                  </a>
                  <button
                    class="copy-btn"
                    onclick={() => copyToClipboard(domain.full_domain, domain.id)}
                    title="Copy domain"
                  >
                    {copiedId === domain.id ? "✓" : "⧉"}
                  </button>
                </div>
              </td>
              <td class="target-cell">
                <div class="target-info">
                  {#if domain.target_type === "instance"}
                    {#if inst}
                      {#if inst.running && inst.healthy}
                        <span class="status-dot running" title="Running"></span>
                      {:else if inst.running && inst.healthy === false}
                        <span class="status-dot unhealthy" title="Unhealthy"></span>
                      {:else if inst.running}
                        <span class="status-dot starting" title="Starting"></span>
                      {:else}
                        <span class="status-dot stopped" title="Stopped"></span>
                      {/if}
                    {:else}
                      <span class="status-dot stopped" title="Unknown"></span>
                    {/if}
                  {:else if domain.target_type === "port"}
                    <span class="status-dot {portActive ? 'running' : 'stopped'}" title="{portActive ? 'Port active' : 'Port inactive'}"></span>
                  {:else}
                    <span class="status-dot running" title="Static files"></span>
                  {/if}
                  <div class="target-details">
                    <span class="target-name">
                      {#if domain.target_type === "instance"}
                        Instance: {inst?.name || domain.target_name || "Unknown"}
                      {:else if domain.target_type === "port"}
                        Port: {domain.target_port ?? domain.target_value}
                      {:else}
                        Static Files {domain.static_browse ? "(browse)" : ""}
                      {/if}
                    </span>
                    <span class="target-meta">
                      {#if domain.target_type === "static"}
                        {domain.static_path || "N/A"}
                      {:else}
                        127.0.0.1:{domain.target_port ?? "N/A"}
                      {/if}
                    </span>
                  </div>
                </div>
              </td>
              <td class="actions-cell">
                <div class="action-buttons">
                  <!-- SSL Toggle -->
                  <button
                    class="icon-btn {domain.ssl_enabled ? 'ssl-enabled' : ''}"
                    onclick={() => toggleDomainSsl(domain)}
                    title={domain.ssl_enabled ? 'SSL Enabled - Click to disable' : 'SSL Disabled - Click to enable'}
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      {#if domain.ssl_enabled}
                        <!-- Locked icon -->
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                        <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
                      {:else}
                        <!-- Unlocked icon -->
                        <rect x="3" y="11" width="18" height="11" rx="2" ry="2"></rect>
                        <path d="M7 11V7a5 5 0 0 1 9.9-1"></path>
                      {/if}
                    </svg>
                  </button>
                  <button
                    class="icon-btn"
                    onclick={() => startEditDomain(domain)}
                    title="Edit"
                  >
                    <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                    </svg>
                  </button>
                  <button
                    class="icon-btn danger"
                    onclick={() => deleteDomain(domain)}
                    title="Delete"
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

{#if showConfigModal && proxyConfig}
  <div class="modal-overlay" onclick={() => showConfigModal = false} onkeydown={(e) => e.key === 'Escape' && (showConfigModal = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-content" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Proxy Configuration</h3>
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
            <span>{proxyConfig.tld}</span>
          </div>
        </div>

        <div class="config-section">
          <h4>Paths</h4>
          <div class="config-grid">
            <span class="label">Routes File:</span>
            <code class="path">{proxyConfig.routes_file}</code>
            <span class="label">Certs Directory:</span>
            <code class="path">{proxyConfig.certs_dir}</code>
          </div>
        </div>

        <div class="config-section">
          <h4>Routes Configuration</h4>
          <pre class="json-content">{JSON.stringify(proxyConfig.routes, null, 2)}</pre>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .domains-section {
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
    padding: 1rem;
    border: 1px solid #e5e5e5;
  }

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

  .new-domain-form {
    background: #f5f5f7;
    border-radius: 8px;
    padding: 1.5rem;
    margin-bottom: 1rem;
  }

  @media (prefers-color-scheme: dark) {
    .new-domain-form {
      background: #1c1c1e;
    }
  }

  .new-domain-form h3 {
    margin: 0 0 1rem;
    font-size: 1rem;
    font-weight: 600;
  }

  .form-row {
    margin-bottom: 1rem;
  }

  .form-row label {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .form-row label span {
    font-size: 0.875rem;
    font-weight: 500;
    color: #86868b;
  }

  .subdomain-input {
    display: flex;
    align-items: center;
  }

  .subdomain-input input {
    flex: 1;
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px 0 0 6px;
    font-size: 0.875rem;
  }

  .subdomain-input .tld-suffix {
    padding: 0.5rem 0.75rem;
    background: #e5e5e5;
    border: 1px solid #d1d1d6;
    border-left: none;
    border-radius: 0 6px 6px 0;
    font-size: 0.875rem;
    color: #86868b;
  }

  @media (prefers-color-scheme: dark) {
    .subdomain-input .tld-suffix {
      background: #3a3a3c;
      border-color: #48484a;
      color: #98989d;
    }
  }

  .form-row select,
  .form-row input[type="number"] {
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
  }

  .subdomain-input input {
    background: white;
  }

  @media (prefers-color-scheme: dark) {
    .form-row select,
    .form-row input[type="number"],
    .subdomain-input input {
      background: #1c1c1e;
      border-color: #48484a;
      color: white;
    }
  }

  .checkbox-label {
    flex-direction: row !important;
    align-items: center;
    gap: 0.5rem !important;
  }

  .form-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
  }

  .domains-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  .domains-table th {
    text-align: left;
    padding: 0.75rem;
    border-bottom: 1px solid #e5e5e5;
    font-weight: 600;
    color: #86868b;
    font-size: 0.75rem;
    text-transform: uppercase;
  }

  .domains-table td {
    padding: 0.75rem;
    border-bottom: 1px solid #f0f0f0;
  }

  @media (prefers-color-scheme: dark) {
    .domains-table th {
      border-bottom-color: #48484a;
    }
    .domains-table td {
      border-bottom-color: #38383a;
    }
  }

  .domain-cell {
    padding: 0.5rem 0.75rem !important;
  }

  .domain-row {
    display: flex;
    align-items: center;
    gap: 0.375rem;
  }

  /* Target cell with icon layout */
  .target-cell {
    padding: 0.5rem 0.75rem !important;
  }

  .target-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  /* Target icon styles removed - using status dots instead */

  .target-details {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
    min-width: 0;
  }

  .target-name {
    font-size: 0.875rem;
    font-weight: 500;
    color: #1d1d1f;
  }

  .target-meta {
    font-size: 0.6875rem;
    font-family: 'SF Mono', Menlo, Monaco, 'Courier New', monospace;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }

  @media (prefers-color-scheme: dark) {
    .target-name {
      color: #f5f5f7;
    }

    .target-meta {
      color: #98989d;
    }
  }

  .ssl-badge {
    display: inline-block;
    padding: 0.125rem 0.375rem;
    border-radius: 3px;
    font-size: 0.625rem;
    font-weight: 600;
    text-transform: uppercase;
  }

  .ssl-badge.enabled {
    background: #34c759;
    color: white;
  }

  .ssl-badge.disabled {
    background: #8e8e93;
    color: white;
  }

  /* Action buttons */
  .actions-cell {
    text-align: right;
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

  .icon-btn.ssl-enabled {
    color: #16a34a;
  }

  .icon-btn.ssl-enabled:hover {
    background: rgba(22, 163, 74, 0.1);
    color: #16a34a;
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
    .icon-btn.ssl-enabled {
      color: #22c55e;
    }
    .icon-btn.ssl-enabled:hover {
      background: rgba(34, 197, 94, 0.2);
      color: #22c55e;
    }
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

  /* Table row hover */
  .domains-table tbody tr:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  @media (prefers-color-scheme: dark) {
    .domains-table tbody tr:hover {
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

  :global(:root[data-theme="light"]) .new-domain-form {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="light"]) .subdomain-input .tld-suffix {
    background: #e5e5e5 !important;
    border-color: #d1d1d6 !important;
    color: #86868b !important;
  }

  :global(:root[data-theme="light"]) .form-row select,
  :global(:root[data-theme="light"]) .form-row input[type="number"],
  :global(:root[data-theme="light"]) .subdomain-input input {
    background: white !important;
    border-color: #d1d1d6 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .status-dot.running { background: #22c55e !important; }

  :global(:root[data-theme="light"]) .target-name {
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"]) .target-meta {
    color: #86868b !important;
  }

  :global(:root[data-theme="light"]) .icon-btn {
    color: #86868b !important;
  }
  :global(:root[data-theme="light"]) .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: #1d1d1f !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1) !important;
    color: #dc2626 !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.ssl-enabled {
    color: #16a34a !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.ssl-enabled:hover {
    background: rgba(22, 163, 74, 0.1) !important;
    color: #16a34a !important;
  }

  :global(:root[data-theme="light"]) .domains-table th {
    border-bottom-color: #e5e5e5 !important;
  }
  :global(:root[data-theme="light"]) .domains-table td {
    border-bottom-color: #f0f0f0 !important;
  }

  :global(:root[data-theme="light"]) .domains-table tbody tr:hover {
    background: rgba(0, 0, 0, 0.04) !important;
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

  :global(:root[data-theme="dark"]) .new-domain-form {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .subdomain-input .tld-suffix {
    background: #3a3a3c !important;
    border-color: #48484a !important;
    color: #98989d !important;
  }

  :global(:root[data-theme="dark"]) .form-row select,
  :global(:root[data-theme="dark"]) .form-row input[type="number"],
  :global(:root[data-theme="dark"]) .subdomain-input input {
    background: #1c1c1e !important;
    border-color: #48484a !important;
    color: white !important;
  }

  :global(:root[data-theme="dark"]) .status-dot.running { background: #22c55e !important; }

  :global(:root[data-theme="dark"]) .target-name {
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .target-meta {
    color: #98989d !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2) !important;
    color: #ef4444 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.ssl-enabled {
    color: #22c55e !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.ssl-enabled:hover {
    background: rgba(34, 197, 94, 0.2) !important;
    color: #22c55e !important;
  }

  :global(:root[data-theme="dark"]) .domains-table tbody tr:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="dark"]) .url-link {
    color: #007aff !important;
  }

  :global(:root[data-theme="dark"]) .domains-table th {
    border-bottom-color: #48484a !important;
  }
  :global(:root[data-theme="dark"]) .domains-table td {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .error-banner {
    background: #3d2020 !important;
    color: #fca5a5 !important;
  }

  /* Secondary button style */
  .btn.secondary {
    background: #e5e5e5;
    color: #1d1d1f;
  }

  .btn.secondary:hover {
    background: #d1d1d6;
  }

  @media (prefers-color-scheme: dark) {
    .btn.secondary {
      background: #3a3a3c;
      color: #f5f5f7;
    }
    .btn.secondary:hover {
      background: #48484a;
    }
  }

  :global(:root[data-theme="light"]) .btn.secondary {
    background: #e5e5e5 !important;
    color: #1d1d1f !important;
  }
  :global(:root[data-theme="light"]) .btn.secondary:hover {
    background: #d1d1d6 !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }
  :global(:root[data-theme="dark"]) .btn.secondary:hover {
    background: #48484a !important;
  }

  /* Spinning animation */
  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .spinning {
    animation: spin 1s linear infinite;
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

  :global(:root[data-theme="light"]) .modal-content {
    background: white !important;
  }

  :global(:root[data-theme="dark"]) .modal-content {
    background: #2c2c2e !important;
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

  :global(:root[data-theme="dark"]) .modal-header {
    border-bottom-color: #48484a !important;
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

  :global(:root[data-theme="light"]) .config-grid .path {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .config-grid .path {
    background: #1c1c1e !important;
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

  :global(:root[data-theme="light"]) .json-content {
    background: #f5f5f7 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="dark"]) .json-content {
    background: #1c1c1e !important;
    color: #f5f5f7 !important;
  }

  tr.dragging { opacity: 0.4; }
  tr.drag-over td { box-shadow: inset 0 2px 0 #007aff; }
  .drag-handle {
    cursor: grab;
    width: 28px;
    padding: 0 6px !important;
    color: #c7c7cc;
    user-select: none;
    font-size: 1rem;
    text-align: center;
  }
  .drag-handle:active { cursor: grabbing; }

  .url-link {
    color: #007aff;
    text-decoration: none;
    font-size: 0.8125rem;
    font-family: 'SF Mono', Menlo, Monaco, 'Courier New', monospace;
  }
  .url-link:hover { text-decoration: underline; }

  .copy-btn {
    background: none;
    border: none;
    cursor: pointer;
    color: #86868b;
    font-size: 0.75rem;
    padding: 0.125rem 0.25rem;
    border-radius: 3px;
    line-height: 1;
    flex-shrink: 0;
  }
  .copy-btn:hover {
    background: rgba(0, 0, 0, 0.06);
    color: #1d1d1f;
  }
  @media (prefers-color-scheme: dark) {
    .copy-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .status-dot.running { background: #22c55e; box-shadow: 0 0 6px rgba(34, 197, 94, 0.5); }
  .status-dot.stopped { background: #9ca3af; }
  .status-dot.starting { background: #f59e0b; animation: pulse 1.5s ease-in-out infinite; }
  .status-dot.unhealthy { background: #ef4444; animation: pulse 1s ease-in-out infinite; }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
</style>
