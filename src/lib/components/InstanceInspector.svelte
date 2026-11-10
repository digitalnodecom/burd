<script lang="ts">
  import Modal from './Modal.svelte';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { X, Copy } from '@lucide/svelte';
  import type { InstanceInfo } from '$lib/types';
  let { target, onClose }: { target: { id: string; name: string; serviceType: string; mode: 'logs' | 'env' | 'info' }; onClose: () => void } = $props();
  let error = $state<string | null>(null);
  // Logs modal
  let logsContent = $state("");
  let logsInstanceName = $state("");
  let logsLoading = $state(false);

  // ENV modal
  let envContent = $state("");
  let envServiceType = $state("");
  let envLoading = $state(false);
  let envCopied = $state(false);

  // Info modal
  let infoContent = $state<InstanceInfo | null>(null);
  let infoInstanceName = $state("");
  let infoServiceType = $state("");
  let infoLoading = $state(false);
  let infoError = $state<string | null>(null);

  async function viewLogs(id: string, name: string) {
    try {
      logsLoading = true;
      logsInstanceName = name;
      logsContent = await invoke<string>("get_instance_logs", { id });
    } catch (e) {
      logsContent = `Error loading logs: ${e}`;
    } finally {
      logsLoading = false;
    }
  }

  async function refreshLogs(id: string) {
    try {
      logsLoading = true;
      logsContent = await invoke<string>("get_instance_logs", { id });
    } catch (e) {
      logsContent = `Error loading logs: ${e}`;
    } finally {
      logsLoading = false;
    }
  }

  async function viewEnv(id: string, serviceType: string) {
    try {
      envLoading = true;
      envServiceType = serviceType;
      envCopied = false;
      envContent = await invoke<string>("get_instance_env", { id });
    } catch (e) {
      envContent = `Error loading ENV: ${e}`;
    } finally {
      envLoading = false;
    }
  }

  async function viewInfo(id: string, name: string, serviceType: string) {
    try {
      infoLoading = true;
      infoInstanceName = name;
      infoServiceType = serviceType;
      infoError = null;
      infoContent = await invoke<InstanceInfo>("get_instance_info", { id });
    } catch (e) {
      infoError = String(e);
      infoContent = null;
    } finally {
      infoLoading = false;
    }
  }

  async function copyInfoValue(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      // Show visual feedback (could enhance with a state variable if desired)
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  }

  async function copyEnvToClipboard() {
    try {
      await navigator.clipboard.writeText(envContent);
      envCopied = true;
      setTimeout(() => { envCopied = false; }, 2000);
    } catch (e) {
      error = `Failed to copy: ${e}`;
    }
  }

  onMount(() => {
    if (target.mode === 'logs') void viewLogs(target.id, target.name);
    if (target.mode === 'env') void viewEnv(target.id, target.serviceType);
    if (target.mode === 'info') void viewInfo(target.id, target.name, target.serviceType);
  });
</script>
{#if error}<p role="alert">{error}</p>{/if}
<!-- Logs Modal -->
{#if target.mode === "logs"}
  <Modal onClose={() => (onClose())} overlayClass="modal-overlay">
    <div class="modal"  role="document">
      <div class="modal-header">
        <h3 class="modal-title">Logs: {logsInstanceName}</h3>
        <button aria-label="Close" class="modal-close" onclick={() => (onClose())}><X size={16} strokeWidth={2} /></button>
      </div>
      <div class="modal-body">
        {#if logsLoading}
          <p class="loading">Loading logs...</p>
        {:else}
          <pre class="logs-content">{logsContent}</pre>
        {/if}
      </div>
      <div class="modal-footer">
        <button
          class="btn small secondary"
          onclick={() => {
            refreshLogs(target.id);
          }}
          disabled={logsLoading}
        >
          Refresh Logs
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- ENV Modal -->
{#if target.mode === "env"}
  <Modal onClose={() => (onClose())} overlayClass="modal-overlay">
    <div class="modal"  role="document">
      <div class="modal-header">
        <div class="modal-title-row">
          <h3>ENV Variables</h3>
          <span class="modal-badge">{envServiceType}</span>
        </div>
        <button aria-label="Close" class="modal-close" onclick={() => (onClose())}><X size={16} strokeWidth={2} /></button>
      </div>
      <div class="modal-body">
        {#if envLoading}
          <p class="loading">Loading ENV variables...</p>
        {:else}
          <pre class="env-content">{envContent}</pre>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn secondary" onclick={() => (onClose())}>
          Close
        </button>
        <button
          class="btn primary"
          onclick={copyEnvToClipboard}
          disabled={envLoading}
        >
          {envCopied ? "Copied!" : "Copy to Clipboard"}
        </button>
      </div>
    </div>
  </Modal>
{/if}

<!-- Instance Info Modal -->
{#if target.mode === "info"}
  <Modal onClose={() => (onClose())} overlayClass="modal-overlay">
    <div class="modal info-modal"  role="document">
      <div class="modal-header">
        <div class="modal-title-row">
          <h3>{infoInstanceName} - Information</h3>
          <span class="modal-badge">{infoServiceType}</span>
          <span class="modal-badge {infoContent?.running ? 'success' : 'warning'}">
            {infoContent?.running ? 'Running' : 'Stopped'}
          </span>
        </div>
        <button aria-label="Close" class="modal-close" onclick={() => (onClose())}><X size={16} strokeWidth={2} /></button>
      </div>

      <div class="modal-body">
        {#if infoLoading}
          <p class="loading">Loading information...</p>
        {:else if infoError}
          <div class="error-message">
            <strong>Error:</strong> {infoError}
          </div>
        {:else if infoContent}
          {#each infoContent.categories as category}
            <div class="info-category">
              <h4 class="category-title">{category.title}</h4>
              <div class="info-grid">
                {#each category.items as item}
                  <div class="info-item">
                    <span class="info-label">{item.label}</span>
                    <div class="info-value-row">
                      <span class="info-value">{item.value}</span>
                      {#if item.copyable}
                        <button
                          class="copy-btn"
                          onclick={() => copyInfoValue(item.value)}
                          title="Copy"
                        >
                          <Copy size={12} strokeWidth={2} />
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn secondary" onclick={() => (onClose())}>
          Close
        </button>
      </div>
    </div>
  </Modal>
{/if}

<style>

  @media (prefers-color-scheme: dark) {

  }

  /* Explicit theme overrides via data-theme attribute */
  :global(:root[data-theme="light"]) {
    color-scheme: light;
  }

  :global(:root[data-theme="dark"]) {
    color-scheme: dark;
  }

  :global(:root[data-theme="light"] body) {
    background: var(--surface-2) !important;
    color: var(--text) !important;
  }

  :global(:root[data-theme="dark"] body) {
    background: #1c1c1e !important;
    color: #f5f5f7 !important;
  }

  @media (prefers-color-scheme: dark) {

  }

  /* Button styles */
  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
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
    color: var(--text);
  }

  .btn.secondary:hover:not(:disabled) {
    background: #d1d1d6;
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

  /* Modal styles */

  .modal {
    background: var(--surface);
    border-radius: 12px;
    width: 90%;
    max-width: 600px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.2);
  }

  @media (prefers-color-scheme: dark) {
    .modal {
      background: #2c2c2e;
    }
  }

  @media (prefers-color-scheme: dark) {
  }

  @media (prefers-color-scheme: dark) {

  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 1rem 1.5rem;
    border-top: 1px solid var(--border);
  }

  @media (prefers-color-scheme: dark) {
    .modal-footer {
      border-top-color: #38383a;
    }
  }

  .logs-content {
    background: #1c1c1e;
    color: #f5f5f7;
    padding: 1rem;
    border-radius: 8px;
    font-family: monospace;
    font-size: 0.75rem;
    line-height: 1.5;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
    max-height: 400px;
    overflow-y: auto;
  }

  .env-content {
    background: #1c1c1e;
    color: #a8e6cf;
    padding: 1rem;
    border-radius: 8px;
    font-family: monospace;
    font-size: 0.8125rem;
    line-height: 1.6;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
    max-height: 400px;
    overflow-y: auto;
  }

  .modal-title-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .modal-title-row h3 {
    margin: 0;
  }

  .modal-badge {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
    font-size: 0.6875rem;
    font-weight: 600;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .modal-badge.success {
    background: linear-gradient(135deg, #16a34a, #15803d);
  }

  .modal-badge.warning {
    background: linear-gradient(135deg, #f59e0b, #d97706);
  }

  /* Info Modal Styles */
  .info-modal {
    max-width: 700px;
    max-height: 80vh;
  }

  .info-category {
    margin-bottom: 1.5rem;
  }

  .info-category:last-child {
    margin-bottom: 0;
  }

  .category-title {
    font-size: 0.75rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .info-grid {
    display: grid;
    gap: 0.75rem;
  }

  .info-item {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 1rem;
    padding: 0.625rem 0.75rem;
    background: var(--surface-2);
    border-radius: 6px;
    align-items: center;
  }

  @media (prefers-color-scheme: dark) {
    .info-item {
      background: #2c2c2e;
    }
  }

  .info-label {
    font-weight: 500;
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .info-value-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .info-value {
    font-family: 'SF Mono', 'Monaco', 'Menlo', monospace;
    font-size: 0.8125rem;
    word-break: break-all;
    flex: 1;
    min-width: 0;
  }

  .copy-btn {
    padding: 4px;
    background: transparent;
    border: none;
    cursor: pointer;
    opacity: 0.6;
    color: #007aff;
    transition: opacity 0.15s ease;
    flex-shrink: 0;
  }

  .copy-btn:hover {
    opacity: 1;
  }

  @media (prefers-color-scheme: dark) {
    .copy-btn {
      color: #0a84ff;
    }
  }

  .error-message {
    background: #ff3b30;
    color: white;
    padding: 1rem;
    border-radius: 8px;
  }

  @media (prefers-color-scheme: dark) {
    .error-message {
      background: #dc2626;
    }
  }

  /* Settings form styles */

  /* Name + Port side by side */

  /* Auto-start toggle — a plain inline row */

  /* Domains — compact */

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  .loading {
    text-align: center;
    color: var(--text-muted);
    padding: 2rem;
  }

  /* Explicit dark theme overrides via data-theme attribute */
  :global(:root[data-theme="dark"] .card) {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"] footer) {
    border-top-color: #38383a;
  }

  :global(:root[data-theme="dark"] .btn.secondary) {
    background: #3a3a3c;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"] .btn.secondary:hover:not(:disabled)) {
    background: #48484a;
  }

  :global(:root[data-theme="dark"] .modal) {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"] .modal-header) {
    border-bottom-color: #38383a;
  }

  :global(:root[data-theme="dark"] .modal-header .close:hover) {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"] .modal-footer) {
    border-top-color: #38383a;
  }

  :global(:root[data-theme="dark"] .settings-hint code) {
    background: #1c1c1e;
  }

  :global(:root[data-theme="dark"] .tld-prefix) {
    background: #3a3a3c;
    border-color: #48484a;
  }

  :global(:root[data-theme="dark"] input),
  :global(:root[data-theme="dark"] select) {
    background: #1c1c1e;
    border-color: #3a3a3c;
  }

  :global(:root[data-theme="dark"] .network-item),
  :global(:root[data-theme="dark"] .instance-item),
  :global(:root[data-theme="dark"] .service-item),
  :global(:root[data-theme="dark"] .domain-item),
  :global(:root[data-theme="dark"] .tunnel-item),
  :global(:root[data-theme="dark"] .server-item),
  :global(:root[data-theme="dark"] .version-item) {
    background: #1c1c1e !important;
  }

  /* Explicit light theme overrides via data-theme attribute */
  :global(:root[data-theme="light"] .card) {
    background: var(--surface) !important;
  }

  :global(:root[data-theme="light"] .card.warning) {
    background: #fef3c7 !important;
  }

  :global(:root[data-theme="light"] footer) {
    border-top-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .btn.secondary) {
    background: #e5e5e5 !important;
    color: var(--text) !important;
  }

  :global(:root[data-theme="light"] .btn.secondary:hover:not(:disabled)) {
    background: #d1d1d6 !important;
  }

  :global(:root[data-theme="light"] .modal) {
    background: var(--surface) !important;
  }

  :global(:root[data-theme="light"] .modal-header) {
    border-bottom-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .modal-footer) {
    border-top-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .settings-hint code) {
    background: var(--surface-2) !important;
  }

  :global(:root[data-theme="light"] .tld-prefix) {
    background: var(--surface-2) !important;
    border-color: #d1d1d6 !important;
  }

  :global(:root[data-theme="light"] input),
  :global(:root[data-theme="light"] select),
  :global(:root[data-theme="light"] textarea) {
    background: var(--surface) !important;
    border-color: #d1d1d6 !important;
    color: var(--text) !important;
  }

  /* Common section elements - light mode */
  :global(:root[data-theme="light"] .network-item),
  :global(:root[data-theme="light"] .instance-item),
  :global(:root[data-theme="light"] .service-item),
  :global(:root[data-theme="light"] .domain-item),
  :global(:root[data-theme="light"] .tunnel-item),
  :global(:root[data-theme="light"] .server-item),
  :global(:root[data-theme="light"] .version-item) {
    background: var(--surface-2) !important;
  }

  :global(:root[data-theme="light"] .form-container),
  :global(:root[data-theme="light"] .network-hint),
  :global(:root[data-theme="light"] .hint-box),
  :global(:root[data-theme="light"] .info-box) {
    background: var(--surface-2) !important;
  }

  :global(:root[data-theme="light"] .small-button) {
    background: #e5e5e5 !important;
    color: var(--text) !important;
  }

  :global(:root[data-theme="light"] .status-card) {
    background: var(--surface) !important;
  }

  :global(:root[data-theme="light"] .status-card.warning) {
    background: #fef3c7 !important;
    border-color: #f59e0b !important;
  }

  :global(:root[data-theme="light"] .status-card.success) {
    background: #dcfce7 !important;
    border-color: #16a34a !important;
  }

  :global(:root[data-theme="light"] .status-card.info) {
    background: #dbeafe !important;
    border-color: #2563eb !important;
  }

  :global(:root[data-theme="light"] .error-banner) {
    background: #fee2e2 !important;
    color: #dc2626 !important;
  }

  :global(:root[data-theme="light"] pre),
  :global(:root[data-theme="light"] code) {
    background: var(--surface-2) !important;
    color: var(--text) !important;
  }

  :global(:root[data-theme="light"] .label),
  :global(:root[data-theme="light"] .hint),
  :global(:root[data-theme="light"] .desc) {
    color: var(--text-muted) !important;
  }

  :global(:root[data-theme="light"] section.card) {
    background: var(--surface) !important;
  }

  /* Error banner explicit overrides */

  /* Dark mode additional fixes */
</style>
