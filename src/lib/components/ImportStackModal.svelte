<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  // Types
  interface StackExport {
    id: string;
    name: string;
    description: string | null;
    schema_version: number;
    created_by: string | null;
    created_at: string;
    updated_at: string;
    services: StackService[];
    domains: StackDomain[];
    requirements: StackRequirements;
  }

  interface StackService {
    ref_id: string;
    service_type: string;
    version: string;
    name: string;
    port: number;
    auto_start: boolean;
    config: Record<string, unknown>;
  }

  interface StackDomain {
    subdomain: string;
    target_ref: string;
    ssl_enabled: boolean;
  }

  interface StackRequirements {
    min_burd_version: string | null;
  }

  interface MissingVersion {
    service_type: string;
    version: string;
    download_size: number | null;
  }

  interface ImportConflict {
    type: "PortInUse" | "NameExists" | "StackIdExists";
    port?: number;
    existing_instance_name?: string;
    new_service_ref?: string;
    name?: string;
    existing_id?: string;
    existing_stack_name?: string;
  }

  interface Stack {
    id: string;
    name: string;
    description: string | null;
    created_at: string;
    updated_at: string;
  }

  interface StackImportPreview {
    config: StackExport;
    missing_versions: MissingVersion[];
    conflicts: ImportConflict[];
    existing_stack: Stack | null;
  }

  interface ConflictResolution {
    type: "ReassignPort" | "RenameService" | "ReplaceExisting" | "Skip" | "UpdateExistingStack";
    service_ref?: string;
    new_port?: number;
    new_name?: string;
  }

  interface ImportResult {
    stack_id: string;
    instances_created: number;
    instances_updated: number;
    domains_created: number;
  }

  let {
    show = false,
    onClose,
    onImported,
  }: {
    show: boolean;
    onClose: () => void;
    onImported: () => void;
  } = $props();

  // State
  type InputMode = "paste" | "file" | "url";
  let inputMode = $state<InputMode>("paste");
  let jsonInput = $state("");
  let urlInput = $state("");
  let loading = $state(false);
  let error = $state<string | null>(null);

  // Preview state
  let preview = $state<StackImportPreview | null>(null);
  let showPreview = $state(false);

  // Conflict resolution state
  let resolutions = $state<Map<string, ConflictResolution>>(new Map());

  // Downloading state
  let downloading = $state(false);
  let downloadProgress = $state<Record<string, number>>({});

  // Importing state
  let importing = $state(false);

  function resetState() {
    jsonInput = "";
    urlInput = "";
    error = null;
    preview = null;
    showPreview = false;
    resolutions = new Map();
    downloading = false;
    downloadProgress = {};
    importing = false;
  }

  function handleClose() {
    resetState();
    onClose();
  }

  async function handleFileSelect(event: Event) {
    const input = event.target as HTMLInputElement;
    if (!input.files?.length) return;

    const file = input.files[0];
    try {
      jsonInput = await file.text();
      inputMode = "paste"; // Switch to paste view to show content
    } catch (e) {
      error = `Failed to read file: ${e}`;
    }
  }

  async function loadFromUrl() {
    if (!urlInput.trim()) {
      error = "Please enter a URL";
      return;
    }

    try {
      loading = true;
      error = null;
      const response = await fetch(urlInput.trim());
      if (!response.ok) {
        throw new Error(`HTTP ${response.status}: ${response.statusText}`);
      }
      jsonInput = await response.text();
      inputMode = "paste"; // Switch to paste view to show content
    } catch (e) {
      error = `Failed to fetch URL: ${e}`;
    } finally {
      loading = false;
    }
  }

  async function loadPreview() {
    if (!jsonInput.trim()) {
      error = "Please paste or load a stack configuration";
      return;
    }

    try {
      loading = true;
      error = null;
      preview = await invoke<StackImportPreview>("preview_stack_import", {
        config_json: jsonInput.trim(),
      });
      showPreview = true;

      // Initialize resolutions for conflicts
      const newResolutions = new Map<string, ConflictResolution>();
      for (const conflict of preview.conflicts) {
        if (conflict.type === "StackIdExists") {
          newResolutions.set("stack", { type: "UpdateExistingStack" });
        } else if (conflict.new_service_ref) {
          // Default: skip the conflicting service
          newResolutions.set(conflict.new_service_ref, {
            type: "Skip",
            service_ref: conflict.new_service_ref,
          });
        }
      }
      resolutions = newResolutions;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function downloadMissingVersions() {
    if (!preview?.missing_versions.length) return;

    downloading = true;
    error = null;

    try {
      for (const mv of preview.missing_versions) {
        downloadProgress = { ...downloadProgress, [mv.service_type]: 0 };
        await invoke("download_binary", {
          service_type: mv.service_type,
          version: mv.version,
        });
        downloadProgress = { ...downloadProgress, [mv.service_type]: 100 };
      }

      // Refresh preview after downloads
      preview = await invoke<StackImportPreview>("preview_stack_import", {
        config_json: jsonInput.trim(),
      });
    } catch (e) {
      error = `Download failed: ${e}`;
    } finally {
      downloading = false;
    }
  }

  function setResolution(conflictKey: string, resolution: ConflictResolution) {
    const newMap = new Map(resolutions);
    newMap.set(conflictKey, resolution);
    resolutions = newMap;
  }

  function getNextAvailablePort(basePort: number): number {
    // Simple logic: just add 1
    return basePort + 1;
  }

  async function performImport() {
    if (!preview) return;

    // Check if there are still missing versions
    if (preview.missing_versions.length > 0) {
      error = "Please download missing versions first";
      return;
    }

    importing = true;
    error = null;

    try {
      const conflictResolutions: ConflictResolution[] = Array.from(resolutions.values());

      const result = await invoke<ImportResult>("import_stack", {
        config_json: jsonInput.trim(),
        conflict_resolutions: conflictResolutions,
      });

      // Success!
      onImported();
      handleClose();
    } catch (e) {
      error = `Import failed: ${e}`;
    } finally {
      importing = false;
    }
  }

  function formatServiceType(type: string): string {
    return type.charAt(0).toUpperCase() + type.slice(1).toLowerCase();
  }
</script>

{#if show}
  <div class="modal-overlay" onclick={handleClose} onkeydown={(e) => e.key === 'Escape' && handleClose()} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Import Stack</h3>
        <button class="close-btn" onclick={handleClose}>&times;</button>
      </div>

      <div class="modal-body">
        {#if error}
          <div class="error-banner">{error}</div>
        {/if}

        {#if !showPreview}
          <!-- Input Mode Selection -->
          <div class="input-tabs">
            <button
              class="tab"
              class:active={inputMode === "paste"}
              onclick={() => (inputMode = "paste")}
            >
              Paste JSON
            </button>
            <button
              class="tab"
              class:active={inputMode === "file"}
              onclick={() => (inputMode = "file")}
            >
              From File
            </button>
            <button
              class="tab"
              class:active={inputMode === "url"}
              onclick={() => (inputMode = "url")}
            >
              From URL
            </button>
          </div>

          <!-- Input Area -->
          <div class="input-area">
            {#if inputMode === "paste"}
              <textarea
                bind:value={jsonInput}
                placeholder="Paste stack configuration JSON here..."
                rows={12}
              ></textarea>
            {:else if inputMode === "file"}
              <div class="file-input-area">
                <input
                  type="file"
                  accept=".json,.burd.json"
                  onchange={handleFileSelect}
                  id="file-input"
                />
                <label for="file-input" class="file-label">
                  <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
                    <polyline points="17 8 12 3 7 8"/>
                    <line x1="12" y1="3" x2="12" y2="15"/>
                  </svg>
                  <span>Choose a .json or .burd.json file</span>
                </label>
                {#if jsonInput}
                  <div class="file-loaded">File loaded successfully</div>
                {/if}
              </div>
            {:else if inputMode === "url"}
              <div class="url-input-area">
                <input
                  type="url"
                  bind:value={urlInput}
                  placeholder="https://example.com/stack.json"
                />
                <button class="btn secondary" onclick={loadFromUrl} disabled={loading}>
                  {loading ? "Loading..." : "Fetch"}
                </button>
              </div>
              {#if jsonInput}
                <div class="file-loaded">Content loaded from URL</div>
              {/if}
            {/if}
          </div>
        {:else if preview}
          <!-- Preview View -->
          <div class="preview-section">
            <div class="stack-info">
              <h4>{preview.config.name}</h4>
              {#if preview.config.description}
                <p class="description">{preview.config.description}</p>
              {/if}
              <div class="meta">
                {preview.config.services.length} services
                {#if preview.config.domains.length > 0}
                  , {preview.config.domains.length} domains
                {/if}
              </div>
            </div>

            <!-- Services List -->
            <div class="services-list">
              <h5>Services</h5>
              {#each preview.config.services as service}
                {@const hasMissing = preview.missing_versions.some(
                  (mv) => mv.service_type === service.service_type && mv.version === service.version
                )}
                {@const hasConflict = preview.conflicts.some(
                  (c) => c.new_service_ref === service.ref_id
                )}
                <div class="service-item" class:warning={hasMissing} class:conflict={hasConflict}>
                  <span class="service-type">{formatServiceType(service.service_type)}</span>
                  <span class="service-name">{service.name}</span>
                  <span class="service-version">{service.version}</span>
                  <span class="service-port">:{service.port}</span>
                  {#if hasMissing}
                    <span class="badge warning">Need Download</span>
                  {:else}
                    <span class="badge success">Ready</span>
                  {/if}
                </div>
              {/each}
            </div>

            <!-- Missing Versions -->
            {#if preview.missing_versions.length > 0}
              <div class="missing-versions">
                <h5>Missing Versions</h5>
                <p class="hint">The following versions need to be downloaded before importing:</p>
                <div class="missing-list">
                  {#each preview.missing_versions as mv}
                    <div class="missing-item">
                      <span>{formatServiceType(mv.service_type)} {mv.version}</span>
                      {#if downloading && downloadProgress[mv.service_type] !== undefined}
                        <span class="download-status">
                          {downloadProgress[mv.service_type] === 100 ? "Done" : "Downloading..."}
                        </span>
                      {/if}
                    </div>
                  {/each}
                </div>
                <button
                  class="btn primary"
                  onclick={downloadMissingVersions}
                  disabled={downloading}
                >
                  {downloading ? "Downloading..." : "Download All Missing Versions"}
                </button>
              </div>
            {/if}

            <!-- Conflicts -->
            {#if preview.conflicts.length > 0}
              <div class="conflicts-section">
                <h5>Conflicts</h5>
                {#each preview.conflicts as conflict}
                  <div class="conflict-item">
                    {#if conflict.type === "PortInUse"}
                      <div class="conflict-desc">
                        Port <strong>{conflict.port}</strong> is already used by "{conflict.existing_instance_name}"
                      </div>
                      <div class="conflict-options">
                        <label>
                          <input
                            type="radio"
                            name="conflict-{conflict.new_service_ref}"
                            checked={resolutions.get(conflict.new_service_ref || "")?.type === "ReassignPort"}
                            onchange={() =>
                              setResolution(conflict.new_service_ref || "", {
                                type: "ReassignPort",
                                service_ref: conflict.new_service_ref,
                                new_port: getNextAvailablePort(conflict.port || 0),
                              })
                            }
                          />
                          Reassign to port {getNextAvailablePort(conflict.port || 0)}
                        </label>
                        <label>
                          <input
                            type="radio"
                            name="conflict-{conflict.new_service_ref}"
                            checked={resolutions.get(conflict.new_service_ref || "")?.type === "ReplaceExisting"}
                            onchange={() =>
                              setResolution(conflict.new_service_ref || "", {
                                type: "ReplaceExisting",
                                service_ref: conflict.new_service_ref,
                              })
                            }
                          />
                          Replace existing instance
                        </label>
                        <label>
                          <input
                            type="radio"
                            name="conflict-{conflict.new_service_ref}"
                            checked={resolutions.get(conflict.new_service_ref || "")?.type === "Skip"}
                            onchange={() =>
                              setResolution(conflict.new_service_ref || "", {
                                type: "Skip",
                                service_ref: conflict.new_service_ref,
                              })
                            }
                          />
                          Skip this service
                        </label>
                      </div>
                    {:else if conflict.type === "NameExists"}
                      <div class="conflict-desc">
                        Name "{conflict.name}" already exists
                      </div>
                      <div class="conflict-options">
                        <label>
                          <input
                            type="radio"
                            name="conflict-{conflict.new_service_ref}"
                            checked={resolutions.get(conflict.new_service_ref || "")?.type === "RenameService"}
                            onchange={() =>
                              setResolution(conflict.new_service_ref || "", {
                                type: "RenameService",
                                service_ref: conflict.new_service_ref,
                                new_name: `${conflict.name} (2)`,
                              })
                            }
                          />
                          Rename to "{conflict.name} (2)"
                        </label>
                        <label>
                          <input
                            type="radio"
                            name="conflict-{conflict.new_service_ref}"
                            checked={resolutions.get(conflict.new_service_ref || "")?.type === "ReplaceExisting"}
                            onchange={() =>
                              setResolution(conflict.new_service_ref || "", {
                                type: "ReplaceExisting",
                                service_ref: conflict.new_service_ref,
                              })
                            }
                          />
                          Replace existing instance
                        </label>
                        <label>
                          <input
                            type="radio"
                            name="conflict-{conflict.new_service_ref}"
                            checked={resolutions.get(conflict.new_service_ref || "")?.type === "Skip"}
                            onchange={() =>
                              setResolution(conflict.new_service_ref || "", {
                                type: "Skip",
                                service_ref: conflict.new_service_ref,
                              })
                            }
                          />
                          Skip this service
                        </label>
                      </div>
                    {:else if conflict.type === "StackIdExists"}
                      <div class="conflict-desc">
                        A stack with this ID already exists: "{conflict.existing_stack_name}"
                      </div>
                      <div class="conflict-options">
                        <label>
                          <input
                            type="radio"
                            name="conflict-stack"
                            checked={resolutions.get("stack")?.type === "UpdateExistingStack"}
                            onchange={() => setResolution("stack", { type: "UpdateExistingStack" })}
                          />
                          Update existing stack
                        </label>
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            {/if}

            <!-- Existing Stack Notice -->
            {#if preview.existing_stack}
              <div class="existing-stack-notice">
                <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"/>
                  <line x1="12" y1="8" x2="12" y2="12"/>
                  <line x1="12" y1="16" x2="12.01" y2="16"/>
                </svg>
                <span>This will update the existing stack "{preview.existing_stack.name}"</span>
              </div>
            {/if}
          </div>
        {/if}
      </div>

      <div class="modal-footer">
        {#if showPreview}
          <button class="btn secondary" onclick={() => (showPreview = false)}>
            Back
          </button>
        {/if}
        <button class="btn secondary" onclick={handleClose}>
          Cancel
        </button>
        {#if !showPreview}
          <button
            class="btn primary"
            onclick={loadPreview}
            disabled={loading || !jsonInput.trim()}
          >
            {loading ? "Loading..." : "Load & Preview"}
          </button>
        {:else}
          <button
            class="btn primary"
            onclick={performImport}
            disabled={importing || (preview?.missing_versions.length ?? 0) > 0}
          >
            {#if importing}
              Importing...
            {:else if (preview?.missing_versions.length ?? 0) > 0}
              Download Versions First
            {:else}
              Import Stack
            {/if}
          </button>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
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
    width: 90%;
    max-width: 600px;
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.25rem;
    border-bottom: 1px solid #e5e5e5;
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

  .modal-body {
    padding: 1.25rem;
    overflow-y: auto;
    flex: 1;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 1rem 1.25rem;
    border-top: 1px solid #e5e5e5;
  }

  .error-banner {
    background: #fee2e2;
    color: #dc2626;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    font-size: 0.875rem;
  }

  .input-tabs {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
  }

  .tab {
    padding: 0.5rem 1rem;
    border: 1px solid #e5e5e5;
    background: white;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.875rem;
    transition: all 0.15s ease;
  }

  .tab:hover {
    background: #f5f5f7;
  }

  .tab.active {
    background: #1d1d1f;
    color: white;
    border-color: #1d1d1f;
  }

  .input-area textarea {
    width: 100%;
    padding: 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 8px;
    font-family: monospace;
    font-size: 0.8125rem;
    resize: vertical;
    min-height: 200px;
  }

  .file-input-area {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 2rem;
    border: 2px dashed #d1d1d6;
    border-radius: 8px;
    text-align: center;
  }

  .file-input-area input[type="file"] {
    display: none;
  }

  .file-label {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    color: #86868b;
  }

  .file-label:hover {
    color: #1d1d1f;
  }

  .file-loaded {
    margin-top: 1rem;
    color: #16a34a;
    font-size: 0.875rem;
  }

  .url-input-area {
    display: flex;
    gap: 0.5rem;
  }

  .url-input-area input {
    flex: 1;
    padding: 0.5rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
  }

  .preview-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }

  .stack-info h4 {
    margin: 0 0 0.25rem 0;
    font-size: 1rem;
    font-weight: 600;
  }

  .stack-info .description {
    margin: 0;
    color: #86868b;
    font-size: 0.875rem;
  }

  .stack-info .meta {
    font-size: 0.75rem;
    color: #86868b;
    margin-top: 0.25rem;
  }

  .services-list h5,
  .missing-versions h5,
  .conflicts-section h5 {
    margin: 0 0 0.5rem 0;
    font-size: 0.875rem;
    font-weight: 600;
  }

  .service-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.75rem;
    background: #f5f5f7;
    border-radius: 6px;
    margin-bottom: 0.5rem;
    font-size: 0.875rem;
  }

  .service-item.warning {
    background: #fef3c7;
  }

  .service-item.conflict {
    background: #fee2e2;
  }

  .service-type {
    font-weight: 500;
    min-width: 80px;
  }

  .service-name {
    flex: 1;
  }

  .service-version {
    font-family: monospace;
    font-size: 0.75rem;
    color: #86868b;
  }

  .service-port {
    font-family: monospace;
    font-size: 0.75rem;
    color: #86868b;
  }

  .badge {
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .badge.success {
    background: #dcfce7;
    color: #16a34a;
  }

  .badge.warning {
    background: #fef3c7;
    color: #b45309;
  }

  .missing-versions {
    background: #fef3c7;
    padding: 1rem;
    border-radius: 8px;
  }

  .missing-versions .hint {
    margin: 0 0 0.75rem 0;
    font-size: 0.8125rem;
    color: #92400e;
  }

  .missing-list {
    margin-bottom: 1rem;
  }

  .missing-item {
    display: flex;
    justify-content: space-between;
    padding: 0.375rem 0;
    font-size: 0.875rem;
  }

  .download-status {
    color: #16a34a;
    font-size: 0.75rem;
  }

  .conflicts-section {
    background: #fee2e2;
    padding: 1rem;
    border-radius: 8px;
  }

  .conflict-item {
    margin-bottom: 1rem;
  }

  .conflict-item:last-child {
    margin-bottom: 0;
  }

  .conflict-desc {
    font-size: 0.875rem;
    margin-bottom: 0.5rem;
  }

  .conflict-options {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
    padding-left: 1rem;
  }

  .conflict-options label {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8125rem;
    cursor: pointer;
  }

  .existing-stack-notice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: #dbeafe;
    border-radius: 8px;
    color: #1e40af;
    font-size: 0.875rem;
  }

  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
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

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  /* Dark mode */
  @media (prefers-color-scheme: dark) {
    .modal {
      background: #2c2c2e;
    }

    .modal-header {
      border-bottom-color: #38383a;
    }

    .close-btn:hover {
      color: #f5f5f7;
    }

    .modal-footer {
      border-top-color: #38383a;
    }

    .error-banner {
      background: #3d2020;
      color: #fca5a5;
    }

    .tab {
      background: #1c1c1e;
      border-color: #38383a;
      color: #f5f5f7;
    }

    .tab:hover {
      background: #38383a;
    }

    .tab.active {
      background: #f5f5f7;
      color: #1d1d1f;
      border-color: #f5f5f7;
    }

    .input-area textarea {
      background: #1c1c1e;
      border-color: #38383a;
      color: #f5f5f7;
    }

    .file-input-area {
      border-color: #38383a;
    }

    .file-label {
      color: #98989d;
    }

    .file-label:hover {
      color: #f5f5f7;
    }

    .url-input-area input {
      background: #1c1c1e;
      border-color: #38383a;
      color: #f5f5f7;
    }

    .service-item {
      background: #1c1c1e;
    }

    .service-item.warning {
      background: #451a03;
    }

    .service-item.conflict {
      background: #3d2020;
    }

    .missing-versions {
      background: #451a03;
    }

    .missing-versions .hint {
      color: #fcd34d;
    }

    .conflicts-section {
      background: #3d2020;
    }

    .existing-stack-notice {
      background: #1e3a5f;
      color: #93c5fd;
    }

    .btn.secondary {
      background: #3a3a3c;
      color: #f5f5f7;
    }

    .btn.secondary:hover:not(:disabled) {
      background: #48484a;
    }
  }

  :global(:root[data-theme="dark"]) .modal {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .modal-header {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .modal-footer {
    border-top-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .tab {
    background: #1c1c1e !important;
    border-color: #38383a !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .tab.active {
    background: #f5f5f7 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="dark"]) .input-area textarea,
  :global(:root[data-theme="dark"]) .url-input-area input {
    background: #1c1c1e !important;
    border-color: #38383a !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .service-item {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }
</style>
