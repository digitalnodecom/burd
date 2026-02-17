<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { save } from "@tauri-apps/plugin-dialog";

  interface Stack {
    id: string;
    name: string;
    description: string | null;
    created_at: string;
    updated_at: string;
  }

  let {
    show = false,
    stack = null,
    onClose,
  }: {
    show: boolean;
    stack: Stack | null;
    onClose: () => void;
  } = $props();

  // Form state
  let exportName = $state("");
  let exportDescription = $state("");
  let includeDomains = $state(true);

  // Action state
  let loading = $state(false);
  let error = $state<string | null>(null);
  let copied = $state(false);
  let exportedJson = $state<string | null>(null);

  // Initialize form when stack changes
  $effect(() => {
    if (stack && show) {
      exportName = stack.name;
      exportDescription = stack.description || "";
      exportedJson = null;
      error = null;
      copied = false;
    }
  });

  function handleClose() {
    exportedJson = null;
    error = null;
    copied = false;
    onClose();
  }

  async function generateExport(): Promise<string | null> {
    if (!stack) return null;

    try {
      loading = true;
      error = null;

      // Get the export JSON from backend
      let json = await invoke<string>("export_stack", {
        request: {
          stack_id: stack.id,
          include_domains: includeDomains,
        }
      });

      // Parse, modify name/description if changed, and re-stringify
      const parsed = JSON.parse(json);
      if (exportName !== stack.name) {
        parsed.name = exportName;
      }
      if (exportDescription !== (stack.description || "")) {
        parsed.description = exportDescription || null;
      }

      return JSON.stringify(parsed, null, 2);
    } catch (e) {
      error = String(e);
      return null;
    } finally {
      loading = false;
    }
  }

  async function handleCopyToClipboard() {
    const json = await generateExport();
    if (!json) return;

    try {
      await navigator.clipboard.writeText(json);
      exportedJson = json;
      copied = true;
      setTimeout(() => {
        copied = false;
      }, 2000);
    } catch (e) {
      error = `Failed to copy: ${e}`;
    }
  }

  async function handleSaveToFile() {
    const json = await generateExport();
    if (!json) return;

    try {
      // Sanitize filename
      const safeName = exportName.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/-+/g, "-").replace(/^-|-$/g, "");
      const defaultPath = `${safeName}.burd.json`;

      const filePath = await save({
        defaultPath,
        filters: [
          { name: "Burd Stack", extensions: ["burd.json", "json"] },
        ],
      });

      if (filePath) {
        // Write file using Tauri fs plugin or invoke a command
        // For now, we'll use the browser's download mechanism as fallback
        // since we're in a Tauri context, we can use invoke
        const blob = new Blob([json], { type: "application/json" });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = filePath.split("/").pop() || defaultPath;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);

        exportedJson = json;
      }
    } catch (e) {
      error = `Failed to save file: ${e}`;
    }
  }

  async function handlePreview() {
    const json = await generateExport();
    if (json) {
      exportedJson = json;
    }
  }
</script>

{#if show && stack}
  <div class="modal-overlay" onclick={handleClose} onkeydown={(e) => e.key === 'Escape' && handleClose()} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Export Stack</h3>
        <button class="close-btn" onclick={handleClose}>&times;</button>
      </div>

      <div class="modal-body">
        {#if error}
          <div class="error-banner">{error}</div>
        {/if}

        <div class="form-group">
          <label>
            <span class="label">Stack Name</span>
            <input type="text" bind:value={exportName} placeholder="My Stack" />
          </label>
        </div>

        <div class="form-group">
          <label>
            <span class="label">Description</span>
            <textarea
              bind:value={exportDescription}
              placeholder="Optional description for the stack..."
              rows={3}
            ></textarea>
          </label>
        </div>

        <div class="form-group checkbox-group">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={includeDomains} />
            <span>Include domain configurations</span>
          </label>
        </div>

        <div class="secrets-warning">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
          </svg>
          <span>Secrets (passwords, API keys, tokens) will be automatically stripped from the export</span>
        </div>

        {#if exportedJson}
          <div class="preview-section">
            <div class="preview-header">
              <span class="preview-label">Preview</span>
              <button class="btn-link" onclick={() => exportedJson = null}>Hide</button>
            </div>
            <pre class="json-preview">{exportedJson}</pre>
          </div>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn secondary" onclick={handleClose}>
          Cancel
        </button>
        {#if !exportedJson}
          <button class="btn secondary" onclick={handlePreview} disabled={loading}>
            Preview
          </button>
        {/if}
        <button
          class="btn secondary"
          onclick={handleSaveToFile}
          disabled={loading || !exportName.trim()}
        >
          <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          Save File
        </button>
        <button
          class="btn primary"
          onclick={handleCopyToClipboard}
          disabled={loading || !exportName.trim()}
        >
          {#if loading}
            Exporting...
          {:else if copied}
            Copied!
          {:else}
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
            </svg>
            Copy JSON
          {/if}
        </button>
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
    max-width: 520px;
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

  .form-group {
    margin-bottom: 1rem;
  }

  .form-group label {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .label {
    font-size: 0.875rem;
    font-weight: 500;
  }

  .form-group input[type="text"],
  .form-group textarea {
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    font-family: inherit;
    resize: vertical;
  }

  .form-group input[type="text"]:focus,
  .form-group textarea:focus {
    outline: none;
    border-color: #007aff;
  }

  .checkbox-group {
    margin-top: 0.5rem;
  }

  .checkbox-label {
    flex-direction: row !important;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    font-size: 0.875rem;
  }

  .checkbox-label input[type="checkbox"] {
    width: 16px;
    height: 16px;
    accent-color: #007aff;
  }

  .secrets-warning {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: #fef3c7;
    border-radius: 8px;
    color: #92400e;
    font-size: 0.8125rem;
    margin-top: 1rem;
  }

  .secrets-warning svg {
    flex-shrink: 0;
    margin-top: 0.125rem;
  }

  .preview-section {
    margin-top: 1rem;
  }

  .preview-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .preview-label {
    font-size: 0.875rem;
    font-weight: 500;
  }

  .btn-link {
    background: none;
    border: none;
    color: #007aff;
    font-size: 0.8125rem;
    cursor: pointer;
    padding: 0;
  }

  .btn-link:hover {
    text-decoration: underline;
  }

  .json-preview {
    background: #1c1c1e;
    color: #a8e6cf;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    font-family: monospace;
    font-size: 0.75rem;
    line-height: 1.5;
    overflow-x: auto;
    white-space: pre;
    max-height: 200px;
    overflow-y: auto;
    margin: 0;
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
    gap: 0.375rem;
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

    .form-group input[type="text"],
    .form-group textarea {
      background: #1c1c1e;
      border-color: #38383a;
      color: #f5f5f7;
    }

    .secrets-warning {
      background: #451a03;
      color: #fcd34d;
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

  :global(:root[data-theme="dark"]) .form-group input[type="text"],
  :global(:root[data-theme="dark"]) .form-group textarea {
    background: #1c1c1e !important;
    border-color: #38383a !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .secrets-warning {
    background: #451a03 !important;
    color: #fcd34d !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }
</style>
