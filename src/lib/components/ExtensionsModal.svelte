<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { X } from "@lucide/svelte";

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
      <h2 class="modal-title">PostgreSQL Extensions</h2>
      <span class="modal-subtitle">{instanceName}</span>
      <button class="modal-close" onclick={onClose} aria-label="Close"><X size={16} strokeWidth={2} /></button>
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
  /* Modal shell comes from app.css. */
  .db-picker { display: flex; flex-direction: column; gap: var(--sp-1); font-size: 0.85rem; margin-bottom: var(--sp-3); }
  .db-picker select { padding: 6px 8px; border-radius: var(--radius-sm); border: 1px solid var(--border-strong); background: var(--surface); color: var(--text); }
  .db-single { font-size: 0.85rem; margin-bottom: var(--sp-3); color: var(--text-muted); }
  .filter { width: 100%; padding: 8px 10px; border-radius: var(--radius-sm); border: 1px solid var(--border-strong); background: var(--surface); color: var(--text); margin-bottom: var(--sp-3); box-sizing: border-box; }
  .ext-list { display: flex; flex-direction: column; gap: var(--sp-1); }
  .ext-row {
    display: flex; align-items: center; gap: var(--sp-3);
    padding: 10px 12px; border-radius: var(--radius-sm);
    background: var(--surface-2);
  }
  .ext-row.on { background: var(--success-bg); }
  .ext-info { flex: 1; min-width: 0; }
  .ext-name { display: flex; align-items: center; gap: 6px; font-weight: 600; font-size: 0.9rem; }
  .ext-ver { font-weight: 400; color: var(--text-muted); font-size: 0.78rem; }
  .tag { font-size: 0.68rem; background: var(--accent); color: var(--accent-contrast); border-radius: 4px; padding: 1px 5px; }
  .ext-comment { font-size: 0.78rem; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .toggle { flex: none; padding: 5px 12px; border-radius: var(--radius-sm); border: 1px solid var(--border-strong); background: transparent; color: var(--text); cursor: pointer; font-size: 0.82rem; }
  .toggle.enabled { background: var(--success); color: #fff; border-color: var(--success); }
  .toggle:disabled { opacity: 0.5; cursor: default; }
  .error-banner { background: var(--danger-bg); color: var(--danger); padding: var(--sp-2) var(--sp-3); border-radius: var(--radius-sm); font-size: 0.82rem; margin-bottom: var(--sp-3); }
  .loading, .empty { color: var(--text-muted); font-size: 0.85rem; }
</style>
