<script lang="ts">
  import Modal from './Modal.svelte';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { open, confirm, message } from '@tauri-apps/plugin-dialog';
  import { X, Lock, LockOpen, Trash2, TriangleAlert } from '@lucide/svelte';
  import { saveInstanceSettings as persistInstanceSettings } from '$lib/api/instances';
  import type { Instance, ServiceInfo, InstanceConfigResponse } from '$lib/types';
  let { instance, instances, serviceTypes, tld, onClose, onRefresh }: {
    instance: Instance; instances: Instance[]; serviceTypes: ServiceInfo[]; tld: string;
    onClose: () => void; onRefresh: () => Promise<void>;
  } = $props();
  let error = $state<string | null>(null);
  // Instance Settings Modal
  let instanceSettingsId = $state("");
  let instanceSettingsName = $state("");
  let instanceSettingsPort = $state(0);
  let instanceSettingsOriginalPort = $state(0);
  let instanceSettingsServiceType = $state("");
  let instanceSettingsVersion = $state("");
  let instanceSettingsOriginalVersion = $state("");
  let instanceSettingsInstalledVersions = $state<string[]>([]);
  let instanceSettingsConfig = $state<Record<string, string>>({});
  let instanceSettingsLoading = $state(false);
  let instanceSettingsSaving = $state(false);
  let instanceSettingsAutoStart = $state(false);

  // Instance Settings - Domain Management
  let instanceSettingsDomains = $state<string[]>([]);
  let instanceSettingsAllDomains = $state<{ id: string; full_domain: string; ssl_enabled: boolean }[]>([]);
  let showDomainForm = $state(false);
  let newDomainSubdomain = $state("");
  let newDomainSsl = $state(true);

  // === Instance Settings ===
  async function openInstanceSettings(instance: Instance) {
    try {
      instanceSettingsLoading = true;
      instanceSettingsId = instance.id;
      instanceSettingsName = instance.name;
      instanceSettingsPort = instance.port;
      instanceSettingsOriginalPort = instance.port;
      instanceSettingsVersion = instance.version;
      instanceSettingsOriginalVersion = instance.version;
      instanceSettingsAutoStart = instance.auto_start;

      const result = await invoke<InstanceConfigResponse>("get_instance_config", { id: instance.id });
      instanceSettingsServiceType = result.service_type;

      // Fetch installed versions for this service type
      try {
        instanceSettingsInstalledVersions = await invoke<string[]>("get_installed_versions", {
          serviceType: result.service_type
        });
      } catch (e) {
        console.error("Failed to fetch installed versions:", e);
        instanceSettingsInstalledVersions = [instance.version]; // Fallback to current version
      }

      const config: Record<string, string> = {};
      const serviceMeta = serviceTypes.find(s => s.id === result.service_type);
      if (serviceMeta?.config_fields) {
        for (const field of serviceMeta.config_fields) {
          const value = result.config[field.key];
          config[field.key] = value !== undefined && value !== null ? String(value) : (field.default || "");
        }
      }
      instanceSettingsConfig = config;

      // Load domains for this instance
      instanceSettingsDomains = instance.mapped_domains;
      const allDomains = await invoke<any[]>("list_domains");
      instanceSettingsAllDomains = allDomains.filter(d =>
        instance.mapped_domains.includes(d.full_domain)
      );
    } catch (e) {
      error = String(e);
    } finally {
      instanceSettingsLoading = false;
    }
  }

  async function saveInstanceSettings() {
    try {
      instanceSettingsSaving = true;
      error = null;

      const instance = instances.find(i => i.id === instanceSettingsId);

      if (instance?.running) {
        error = "Stop the instance before saving settings, then start it again after saving.";
        await message(error, { title: "Stop Instance First", kind: "info" });
        return;
      }
      await persistInstanceSettings({
        id: instanceSettingsId, name: instanceSettingsName.trim(), port: instanceSettingsPort,
        version: instanceSettingsVersion, config: instanceSettingsConfig, autoStart: instanceSettingsAutoStart,
      });
      // Clear domain form state
      clearDomainForm();
      onClose();

      await onRefresh();
    } catch (e) {
      error = String(e);
    } finally {
      instanceSettingsSaving = false;
    }
  }

  // === Instance Settings - Domain Management Functions ===
  async function createDomainForInstance() {
    try {
      const subdomain = newDomainSubdomain.trim().toLowerCase();
      if (!subdomain) {
        await message("Please enter a subdomain", { title: "Invalid Input", kind: "error" });
        return;
      }

      const request = {
        subdomain: subdomain,
        target_type: "instance",
        target_value: instanceSettingsId,
        ssl_enabled: newDomainSsl
      };

      await invoke("create_domain", { request });
      await refreshInstanceSettings();
      clearDomainForm();
    } catch (e) {
      await message(String(e), { title: "Error Creating Domain", kind: "error" });
    }
  }

  async function deleteDomainFromInstance(domainName: string) {
    try {
      const domain = instanceSettingsAllDomains.find(d => d.full_domain === domainName);
      if (!domain) return;

      const confirmed = await confirm(`Delete ${domainName}?`, {
        title: "Delete Domain",
        kind: "warning"
      });
      if (!confirmed) return;

      await invoke("delete_domain", { id: domain.id });
      await refreshInstanceSettings();
    } catch (e) {
      await message(String(e), { title: "Error Deleting Domain", kind: "error" });
    }
  }

  async function toggleDomainSsl(domainName: string) {
    try {
      const domain = instanceSettingsAllDomains.find(d => d.full_domain === domainName);
      if (!domain) return;

      await invoke("update_domain_ssl", {
        id: domain.id,
        sslEnabled: !domain.ssl_enabled
      });
      await refreshInstanceSettings();
    } catch (e) {
      await message(String(e), { title: "Error Toggling SSL", kind: "error" });
    }
  }

  async function refreshInstanceSettings() {
    try {
      // Reload instance data
      const allInstances = await invoke<Instance[]>("list_instances");
      const instance = allInstances.find(i => i.id === instanceSettingsId);
      if (!instance) return;

      instanceSettingsDomains = instance.mapped_domains;

      // Reload all domains to get IDs
      const allDomains = await invoke<any[]>("list_domains");
      instanceSettingsAllDomains = allDomains.filter(d =>
        instance.mapped_domains.includes(d.full_domain)
      );
    } catch (e) {
      console.error("Failed to refresh instance settings:", e);
    }
  }

  function clearDomainForm() {
    showDomainForm = false;
    newDomainSubdomain = "";
    newDomainSsl = true;
  }

  async function browseFolder(fieldKey: string) {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select Folder" });
      if (selected && typeof selected === "string") {
        instanceSettingsConfig = { ...instanceSettingsConfig, [fieldKey]: selected };
      }
    } catch (e) {
      error = String(e);
    }
  }

  function getInstanceServiceMeta(): ServiceInfo | undefined {
    return serviceTypes.find(s => s.id === instanceSettingsServiceType);
  }

  onMount(() => { void openInstanceSettings(instance); });
</script>
  <Modal onClose={() => (onClose())} overlayClass="modal-overlay">
    <div class="modal settings-modal"  role="document">
      <div class="modal-header">
        <h3 class="modal-title">Settings: {instanceSettingsName}</h3>
        <button aria-label="Close" class="modal-close" onclick={() => (onClose())}><X size={16} strokeWidth={2} /></button>
      </div>
      <div class="modal-body">
        {#if error}<p class="error-message" role="alert">{error}</p>{/if}
        {#if instanceSettingsLoading}
          <p class="loading">Loading configuration...</p>
        {:else}
          <div class="settings-row">
            <div class="settings-group">
              <label>
                <span class="settings-label">Name</span>
                <input
                  type="text"
                  bind:value={instanceSettingsName}
                  placeholder="Instance name..."
                />
              </label>
            </div>
            <div class="settings-group port-group">
              <label>
                <span class="settings-label">Port</span>
                <input
                  type="number"
                  min="1024"
                  max="65535"
                  bind:value={instanceSettingsPort}
                  placeholder="Port..."
                />
              </label>
            </div>
          </div>
          {#if instanceSettingsPort !== instanceSettingsOriginalPort}
            <p class="version-warning"><TriangleAlert size={13} strokeWidth={2} /> Stop the instance before saving a port change.</p>
          {/if}

          <label class="setting-toggle">
            <input
              type="checkbox"
              checked={instanceSettingsAutoStart}
              onchange={(e) => (instanceSettingsAutoStart = e.currentTarget.checked)}
            />
            <span class="setting-toggle-title">Start automatically when Burd launches</span>
          </label>

          <!-- Version Selector -->
          <div class="settings-group">
            <label>
              <span class="settings-label">Version</span>
              <select bind:value={instanceSettingsVersion}>
                {#each instanceSettingsInstalledVersions as ver}
                  <option value={ver}>
                    {ver}{ver === instanceSettingsOriginalVersion ? " (current)" : ""}
                  </option>
                {/each}
              </select>
            </label>
            {#if instanceSettingsVersion !== instanceSettingsOriginalVersion}
              <p class="version-warning"><TriangleAlert size={13} strokeWidth={2} /> Changing version requires restart. Data compatibility not guaranteed.</p>
            {/if}
          </div>

          {@const serviceMeta = getInstanceServiceMeta()}
          {#if serviceMeta?.config_fields && serviceMeta.config_fields.length > 0}
            <form onsubmit={(e) => { e.preventDefault(); saveInstanceSettings(); }}>
              {#each serviceMeta.config_fields as field (field.key)}
                <div class="settings-group">
                  <label>
                    <span class="settings-label">
                      {field.label}
                      {#if field.required}
                        <span class="required">*</span>
                      {/if}
                    </span>
                    {#if field.field_type === "folder"}
                      <div class="folder-input-group">
                        <input
                          type="text"
                          value={instanceSettingsConfig[field.key] || ""}
                          oninput={(e) => { instanceSettingsConfig = { ...instanceSettingsConfig, [field.key]: e.currentTarget.value }; }}
                          placeholder={field.default || "Select a folder..."}
                          class="folder-input"
                        />
                        <button type="button" class="btn secondary small browse-btn" onclick={() => browseFolder(field.key)}>
                          Browse
                        </button>
                      </div>
                    {:else if field.field_type === "password"}
                      <input
                        type="password"
                        value={instanceSettingsConfig[field.key] || ""}
                        oninput={(e) => { instanceSettingsConfig = { ...instanceSettingsConfig, [field.key]: e.currentTarget.value }; }}
                        placeholder={field.default ? "(using default)" : "Enter value..."}
                      />
                    {:else}
                      <input
                        type="text"
                        value={instanceSettingsConfig[field.key] || ""}
                        oninput={(e) => { instanceSettingsConfig = { ...instanceSettingsConfig, [field.key]: e.currentTarget.value }; }}
                        placeholder={field.default || "Enter value..."}
                      />
                    {/if}
                  </label>
                </div>
              {/each}
            </form>
          {:else}
            <p class="settings-empty">No configurable settings for this service type.</p>
          {/if}

          <!-- Domains Section -->
          {#if !instanceSettingsLoading}
            <div class="domains-section">
              <div class="domains-head">
                <h4 class="domains-title">Domains</h4>
                {#if !showDomainForm}
                  <button class="btn secondary small" onclick={() => showDomainForm = true}>+ Add</button>
                {/if}
              </div>

              {#if instanceSettingsDomains.length > 0}
                <div class="domains-list">
                  {#each instanceSettingsDomains as domainName}
                    {@const domain = instanceSettingsAllDomains.find(d => d.full_domain === domainName)}
                    <div class="domain-row">
                      <span class="domain-name">{domainName}</span>
                      {#if domain?.ssl_enabled}<span class="ssl-badge">SSL</span>{/if}
                      <span class="domain-spacer"></span>
                      <button
                        class="domain-action"
                        class:ssl-on={domain?.ssl_enabled}
                        onclick={() => toggleDomainSsl(domainName)}
                        title={domain?.ssl_enabled ? "Disable SSL" : "Enable SSL"}
                      >
                        {#if domain?.ssl_enabled}<Lock size={14} strokeWidth={2} />{:else}<LockOpen size={14} strokeWidth={2} />{/if}
                      </button>
                      <button
                        class="domain-action danger"
                        onclick={() => deleteDomainFromInstance(domainName)}
                        title="Delete domain"
                      >
                        <Trash2 size={14} strokeWidth={2} />
                      </button>
                    </div>
                  {/each}
                </div>
              {:else if !showDomainForm}
                <p class="settings-empty">No domains attached.</p>
              {/if}

              {#if showDomainForm}
                <div class="domain-form">
                  <div class="domain-form-field">
                    <input
                      type="text"
                      bind:value={newDomainSubdomain}
                      placeholder="subdomain"
                      class="domain-sub-input"
                    />
                    <span class="domain-tld">.{tld}</span>
                  </div>
                  <label class="domain-ssl-check">
                    <input type="checkbox" bind:checked={newDomainSsl} />
                    <span>SSL</span>
                  </label>
                  <button class="btn primary small" onclick={createDomainForInstance}>Create</button>
                  <button class="btn secondary small" onclick={clearDomainForm}>Cancel</button>
                </div>
              {/if}
            </div>
          {/if}
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn secondary" onclick={() => (onClose())}>
          Cancel
        </button>
        <button
          class="btn primary"
          onclick={saveInstanceSettings}
          disabled={instanceSettingsSaving || instanceSettingsLoading}
        >
          {instanceSettingsSaving ? "Saving..." : "Save Settings"}
        </button>
      </div>
    </div>
  </Modal>

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

  /* Info Modal Styles */

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

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
  .settings-group {
    margin-bottom: 0.85rem;
  }

  /* Name + Port side by side */
  .settings-row {
    display: grid;
    grid-template-columns: 1fr 0.5fr;
    gap: 0.75rem;
  }
  .settings-row .settings-group {
    margin-bottom: 0.85rem;
  }
  .port-group input {
    width: 100%;
    box-sizing: border-box;
  }

  .version-warning {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin: -0.35rem 0 0.85rem;
    font-size: 0.8rem;
    color: var(--warning, #d97706);
  }
  .version-warning :global(svg) {
    flex: none;
  }

  .settings-group label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .settings-label {
    font-weight: 500;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .settings-label .required {
    color: var(--danger);
  }

  /* Auto-start toggle — a plain inline row */
  .setting-toggle {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 0.55rem;
    cursor: pointer;
    margin-bottom: 0.85rem;
  }
  .setting-toggle input {
    margin: 0;
    width: 16px;
    height: 16px;
    flex: none;
    accent-color: var(--accent);
  }
  .setting-toggle-title {
    font-size: 0.9rem;
    color: var(--text);
  }

  .settings-empty {
    font-size: 0.83rem;
    color: var(--text-muted);
    padding: 0.5rem 0;
    margin: 0;
  }

  /* Domains — compact */
  .domains-section {
    margin-top: 1rem;
    padding-top: 0.9rem;
    border-top: 1px solid var(--border);
  }
  .domains-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.6rem;
  }
  .domains-title {
    margin: 0;
    font-size: 0.9rem;
    font-weight: 600;
  }
  .domains-list {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .domain-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }
  .domain-name {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 0.85rem;
  }
  .domain-spacer {
    flex: 1;
  }
  .ssl-badge {
    padding: 0.1rem 0.4rem;
    background: var(--success);
    color: #fff;
    font-size: 0.68rem;
    border-radius: 4px;
    font-weight: 600;
  }
  .domain-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0.25rem;
    border-radius: var(--radius-sm);
    line-height: 1;
  }
  .domain-action:hover {
    background: var(--border);
    color: var(--text);
  }
  .domain-action.ssl-on {
    color: var(--success);
  }
  .domain-action.danger:hover {
    background: var(--danger-bg);
    color: var(--danger);
  }
  .domain-form {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.6rem;
    padding: 0.6rem;
    background: var(--surface-2);
    border-radius: var(--radius-sm);
  }
  .domain-form-field {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex: 1;
    min-width: 140px;
  }
  .domain-sub-input {
    flex: 1;
    min-width: 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .domain-tld {
    color: var(--text-muted);
    font-size: 0.85rem;
  }
  .domain-ssl-check {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.85rem;
    cursor: pointer;
    white-space: nowrap;
  }

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  .folder-input-group {
    display: flex;
    gap: 0.5rem;
  }

  .folder-input {
    flex: 1;
  }

  input,
  select {
    padding: 0.5rem;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    font-size: 0.875rem;
    line-height: 1.4;
    background: var(--surface);
    color: inherit;
  }

  @media (prefers-color-scheme: dark) {
    input,
    select {
      background: #1c1c1e;
      border-color: #3a3a3c;
    }
  }

  input:focus,
  select:focus {
    outline: none;
    border-color: #007aff;
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
