<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  interface ExtensionInfo {
    name: string;
    default_version: string;
    installed: boolean;
    installed_version: string | null;
    comment: string;
  }

  let {
    instanceId,
    instanceName,
    onClose,
  }: {
    instanceId: string;
    instanceName: string;
    onClose: () => void;
  } = $props();

  let databases = $state<string[]>([]);
  let selectedDb = $state<string>("");
  let extensions = $state<ExtensionInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state<Record<string, boolean>>({});
  let filter = $state("");

  // Extensions Burd bundles that users most often want — surfaced first.
  const featured = new Set(["vector", "pg_partman", "postgis", "pgcrypto", "uuid-ossp", "hstore"]);

  const shown = $derived(
    extensions
      .filter((e) => !filter || e.name.toLowerCase().includes(filter.toLowerCase()))
      .sort((a, b) => {
        // installed first, then featured, then alpha
        if (a.installed !== b.installed) return a.installed ? -1 : 1;
        const af = featured.has(a.name), bf = featured.has(b.name);
        if (af !== bf) return af ? -1 : 1;
        return a.name.localeCompare(b.name);
      })
  );

  async function loadDatabases() {
    loading = true;
    error = null;
    try {
      databases = await invoke<string[]>("list_instance_databases", { instanceId });
      if (databases.length > 0) {
        selectedDb = databases[0];
        await loadExtensions();
      } else {
        loading = false;
      }
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  async function loadExtensions() {
    loading = true;
    error = null;
    try {
      extensions = await invoke<ExtensionInfo[]>("list_database_extensions", {
        instanceId,
        database: selectedDb,
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function toggle(ext: ExtensionInfo) {
    busy = { ...busy, [ext.name]: true };
    error = null;
    try {
      await invoke("set_database_extension", {
        instanceId,
        database: selectedDb,
        extension: ext.name,
        enabled: !ext.installed,
      });
      await loadExtensions();
    } catch (e) {
      error = String(e);
    } finally {
      busy = { ...busy, [ext.name]: false };
    }
  }

  $effect(() => {
    loadDatabases();
  });
</script>

<div class="modal-overlay" onclick={onClose} onkeydown={(e) => e.key === "Escape" && onClose()} role="dialog" aria-modal="true" tabindex="-1">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
    <div class="modal-header">
      <h2>PostgreSQL Extensions</h2>
      <span class="subtitle">{instanceName}</span>
      <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
    </div>

    <div class="modal-body">
      {#if databases.length > 1}
        <label class="db-picker">
          Database
          <select bind:value={selectedDb} onchange={loadExtensions}>
            {#each databases as db}
              <option value={db}>{db}</option>
            {/each}
          </select>
        </label>
      {:else if databases.length === 1}
        <div class="db-single">Database: <strong>{selectedDb}</strong></div>
      {/if}

      {#if databases.length === 0 && !loading}
        <p class="empty">No databases yet. Create one first, then enable extensions on it.</p>
      {/if}

      {#if error}
        <div class="error-banner">{error}</div>
      {/if}

      {#if databases.length > 0}
        <input class="filter" type="text" placeholder="Filter extensions…" bind:value={filter} />

        {#if loading}
          <p class="loading">Loading…</p>
        {:else}
          <div class="ext-list">
            {#each shown as ext (ext.name)}
              <div class="ext-row" class:on={ext.installed}>
                <div class="ext-info">
                  <div class="ext-name">
                    {ext.name}
                    {#if featured.has(ext.name)}<span class="tag">bundled</span>{/if}
                    <span class="ext-ver">{ext.installed_version ?? ext.default_version}</span>
                  </div>
                  {#if ext.comment}<div class="ext-comment">{ext.comment}</div>{/if}
                </div>
                <button
                  class="toggle"
                  class:enabled={ext.installed}
                  onclick={() => toggle(ext)}
                  disabled={busy[ext.name]}
                  title={ext.installed ? "Disable (DROP EXTENSION)" : "Enable (CREATE EXTENSION)"}
                >
                  {busy[ext.name] ? "…" : ext.installed ? "Enabled" : "Enable"}
                </button>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }
  .modal {
    background: var(--bg, #fff);
    color: inherit;
    border-radius: 12px;
    width: min(560px, 92vw);
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }
  .modal-header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 16px 20px;
    border-bottom: 1px solid rgba(0, 0, 0, 0.1);
  }
  .modal-header h2 { font-size: 1.1rem; margin: 0; }
  .subtitle { color: #86868b; font-size: 0.85rem; }
  .close-btn { margin-left: auto; border: none; background: none; cursor: pointer; font-size: 1rem; opacity: 0.6; }
  .close-btn:hover { opacity: 1; }
  .modal-body { padding: 16px 20px; overflow-y: auto; }
  .db-picker { display: flex; flex-direction: column; gap: 4px; font-size: 0.85rem; margin-bottom: 12px; }
  .db-picker select { padding: 6px 8px; border-radius: 6px; }
  .db-single { font-size: 0.85rem; margin-bottom: 12px; color: #86868b; }
  .filter { width: 100%; padding: 8px 10px; border-radius: 6px; border: 1px solid rgba(0,0,0,0.15); margin-bottom: 12px; box-sizing: border-box; }
  .ext-list { display: flex; flex-direction: column; gap: 6px; }
  .ext-row {
    display: flex; align-items: center; gap: 12px;
    padding: 10px 12px; border-radius: 8px;
    background: color-mix(in srgb, currentColor 4%, transparent);
  }
  .ext-row.on { background: color-mix(in srgb, #34c759 12%, transparent); }
  .ext-info { flex: 1; min-width: 0; }
  .ext-name { display: flex; align-items: center; gap: 6px; font-weight: 600; font-size: 0.9rem; }
  .ext-ver { font-weight: 400; color: #86868b; font-size: 0.78rem; }
  .tag { font-size: 0.68rem; background: var(--accent, #4f8cff); color: #fff; border-radius: 4px; padding: 1px 5px; }
  .ext-comment { font-size: 0.78rem; color: #86868b; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .toggle { flex: none; padding: 5px 12px; border-radius: 6px; border: 1px solid rgba(0,0,0,0.15); background: transparent; cursor: pointer; font-size: 0.82rem; }
  .toggle.enabled { background: #34c759; color: #fff; border-color: #34c759; }
  .toggle:disabled { opacity: 0.5; cursor: default; }
  .error-banner { background: #ffebe9; color: #b00; padding: 8px 10px; border-radius: 6px; font-size: 0.82rem; margin-bottom: 10px; }
  .loading, .empty { color: #86868b; font-size: 0.85rem; }
</style>
