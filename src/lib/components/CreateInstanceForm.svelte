<script lang="ts">
  import type { ServiceInfo } from '$lib/types';
  let { serviceTypes, installedVersions, tld, onCreate, onServiceTypeChange, onNavigateToServices, onCreated }: {
    serviceTypes: ServiceInfo[]; installedVersions: Record<string, string[]>; tld: string;
    onCreate: (name: string, port: number, type: string, version: string, config: Record<string, string>, domain?: string | null) => void | Promise<void>;
    onServiceTypeChange: (type: string) => { port: number };
    onNavigateToServices: () => void; onCreated: () => void;
  } = $props();
  let error = $state<string | null>(null);
  let newName = $state("");
  let newServiceType = $state("");
  let newPort = $state(7700);
  let newVersion = $state("");
  let newConfigValue = $state("");
  let newDomain = $state("");  // Custom domain subdomain
  let creating = $state(false);

  // Initialize form with first service type
  $effect(() => {
    if (serviceTypes.length > 0 && !newServiceType) {
      newServiceType = serviceTypes[0].id;
      newPort = serviceTypes[0].default_port;
    }
  });

  // Update version when service type or installed versions change
  $effect(() => {
    const versions = installedVersions[newServiceType] || [];
    if (versions.length > 0 && !versions.includes(newVersion)) {
      newVersion = versions[0];
    }
  });

  function getSelectedServiceMeta(): ServiceInfo | undefined {
    return serviceTypes.find(s => s.id === newServiceType);
  }

  function handleServiceTypeChange() {
    const result = onServiceTypeChange(newServiceType);
    newPort = result.port;
    newConfigValue = "";
    // Update version
    const versions = installedVersions[newServiceType] || [];
    newVersion = versions.length > 0 ? versions[0] : "";
  }

  async function handleCreate() {
    // Validate subdomain format if provided
    if (newDomain && !/^[a-z0-9.-]+$/.test(newDomain)) {
      alert("Domain must contain only lowercase letters, numbers, hyphens, and periods");
      return;
    }

    creating = true;
    try {
      const configObj: Record<string, string> = {};
      const meta = getSelectedServiceMeta();
      if (meta?.config_fields?.[0] && newConfigValue) {
        configObj[meta.config_fields[0].key] = newConfigValue;
      }
      await onCreate(newName, newPort, newServiceType, newVersion, configObj, newDomain || null);
      // Reset form
      newName = "";
      newConfigValue = "";
      newDomain = "";
      onCreated();
    } catch (e) {
      error = String(e);
    } finally {
      creating = false;
    }
  }

</script>
    <form class="create-form" onsubmit={(e) => { e.preventDefault(); handleCreate(); }}>
      {#if error}<p class="error-message" role="alert">{error}</p>{/if}
      <div class="form-row">
        <label>
          <span>Service Type</span>
          <select bind:value={newServiceType} onchange={handleServiceTypeChange}>
            {#each serviceTypes as svc (svc.id)}
              <option value={svc.id}>{svc.display_name}</option>
            {/each}
          </select>
        </label>
        <label>
          <span>Version</span>
          {#if installedVersions[newServiceType]?.length > 0}
            <select bind:value={newVersion}>
              {#each installedVersions[newServiceType] as ver (ver)}
                <option value={ver}>{ver}</option>
              {/each}
            </select>
          {:else}
            <select disabled>
              <option>No versions installed</option>
            </select>
          {/if}
        </label>
        <label>
          <span>Name</span>
          <input
            type="text"
            bind:value={newName}
            placeholder="Development"
            required
          />
        </label>
        <label>
          <span>Domain <span class="optional-label">(optional)</span></span>
          <div class="domain-input-group">
            <input
              type="text"
              bind:value={newDomain}
              placeholder="my-app"
              class="domain-input"
              pattern="[a-z0-9](([a-z0-9]|-)*[a-z0-9])?([.][a-z0-9](([a-z0-9]|-)*[a-z0-9])?)*"
            />
            <span class="domain-suffix">.{tld}</span>
          </div>
          <span class="field-hint">
            Leave empty to use instance name. Only lowercase letters, numbers, hyphens, and periods.
          </span>
        </label>
        <label>
          <span>Port</span>
          <input
            type="number"
            bind:value={newPort}
            min="1024"
            max="65535"
            required
          />
        </label>
        {#if getSelectedServiceMeta()?.config_fields?.[0]}
          <label>
            <span>{getSelectedServiceMeta()?.config_fields[0].label} (optional)</span>
            <input
              type={getSelectedServiceMeta()?.config_fields[0].field_type === 'password' ? 'password' : 'text'}
              bind:value={newConfigValue}
              placeholder="Leave empty for default"
            />
          </label>
        {/if}
      </div>
      <button class="btn primary" type="submit" disabled={creating || !newVersion || installedVersions[newServiceType]?.length === 0}>
        {creating ? "Creating..." : "Create Instance"}
      </button>
      {#if installedVersions[newServiceType]?.length === 0}
        <p class="form-hint">Download a {getSelectedServiceMeta()?.display_name} version first. <button type="button" class="btn secondary small" onclick={onNavigateToServices}>Open Services</button></p>
      {/if}
    </form>

<style>

  @media (prefers-color-scheme: dark) {

  }

  .create-form {
    background: var(--surface);
    border-radius: var(--radius);
    padding: var(--sp-6);
    border: 1px solid var(--border);
  }

  .form-row {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 1rem;
    margin-bottom: 1rem;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  label span {
    font-size: 0.875rem;
    font-weight: 500;
  }

  input, select {
    padding: 0.5rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    font-size: 0.875rem;
    background: var(--surface);
    color: var(--text);
  }

  @media (prefers-color-scheme: dark) {
    input, select {
      background: #1c1c1e;
      border-color: #3a3a3c;
    }
  }

  input:focus, select:focus {
    outline: none;
    border-color: #007aff;
  }

  .form-hint {
    margin: 0.5rem 0 0;
    font-size: 0.875rem;
    color: #ff9500;
  }

  .domain-input-group {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .domain-input {
    flex: 1;
  }

  .domain-suffix {
    color: var(--text-muted);
    font-size: 0.875rem;
    font-family: monospace;
  }

  .optional-label {
    color: var(--text-muted);
    font-size: 0.75rem;
    font-weight: 400;
  }

  .field-hint {
    display: block;
    margin-top: 0.25rem;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  :global(body.dragging-instance) {
    cursor: grabbing !important;
    user-select: none;
  }

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
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
    gap: 0.25rem;
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

  /* Icon button styles */

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @media (prefers-color-scheme: dark) {

  }

  /* Light mode explicit overrides */

  :global(:root[data-theme="light"]) .create-form {
    background: var(--surface) !important;
  }

  /* Dark mode explicit overrides */

  :global(:root[data-theme="dark"]) .create-form {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) input,
  :global(:root[data-theme="dark"]) select {
    background: #1c1c1e !important;
    border-color: #3a3a3c !important;
  }

  /* Header actions */

  /* Stack group styles */

  /* Allow header to receive drag events while children handle clicks */

  /* Selection bar */

  /* Checkbox column */

  /* Modal styles */

  /* Mapped domains display */

  /* Drag and Drop Styles */

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  /* DnD Error notification */

  @keyframes slideIn {
    from {
      transform: translateY(-10px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }

  @media (prefers-color-scheme: dark) {

  }

  /* Drag-and-drop uses mouse events, no special pointer-events needed */
</style>
