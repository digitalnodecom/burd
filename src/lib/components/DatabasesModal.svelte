<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { X } from "@lucide/svelte";

  interface DatabaseInfo {
    name: string;
    size: number | null;
    tables: number | null;
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

  let databases = $state<DatabaseInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  function formatBytes(n: number | null): string {
    if (n == null) return "—";
    if (n < 1024) return `${n} B`;
    const units = ["KB", "MB", "GB", "TB"];
    let value = n / 1024;
    let i = 0;
    while (value >= 1024 && i < units.length - 1) {
      value /= 1024;
      i++;
    }
    return `${value.toFixed(value < 10 ? 1 : 0)} ${units[i]}`;
  }

  // Largest first, so the biggest consumers are obvious.
  const shown = $derived([...databases].sort((a, b) => (b.size ?? 0) - (a.size ?? 0)));
  const total = $derived(databases.reduce((sum, d) => sum + (d.size ?? 0), 0));
  const maxSize = $derived(Math.max(1, ...databases.map((d) => d.size ?? 0)));

  async function load() {
    loading = true;
    error = null;
    try {
      databases = await invoke<DatabaseInfo[]>("list_instance_database_details", { instanceId });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    load();
  });
</script>

<div
  class="modal-overlay"
  onclick={onClose}
  onkeydown={(e) => e.key === "Escape" && onClose()}
  role="dialog"
  aria-modal="true"
  tabindex="-1"
>
  <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
    <div class="modal-header">
      <h2 class="modal-title">Databases</h2>
      <span class="modal-subtitle">{instanceName}</span>
      <button class="modal-close" onclick={onClose} aria-label="Close"><X size={16} strokeWidth={2} /></button>
    </div>

    <div class="modal-body">
      {#if error}
        <div class="error-banner">{error}</div>
      {/if}

      {#if loading}
        <p class="loading">Loading…</p>
      {:else if databases.length === 0}
        <p class="empty">No databases yet. Create one to see its size here.</p>
      {:else}
        <div class="summary">
          <span>{databases.length} database{databases.length === 1 ? "" : "s"}</span>
          <span class="total">{formatBytes(total)} total</span>
        </div>
        <div class="db-list">
          {#each shown as db (db.name)}
            <div class="db-row">
              <div class="db-info">
                <span class="db-name">{db.name}</span>
                <span class="db-size">{formatBytes(db.size)}</span>
              </div>
              <div class="bar">
                <div class="bar-fill" style="width: {((db.size ?? 0) / maxSize) * 100}%"></div>
              </div>
            </div>
          {/each}
        </div>
        <p class="note">Sizes are on-disk estimates reported by the database server.</p>
      {/if}
    </div>
  </div>
</div>

<style>
  /* Modal shell (overlay / card / header / body / close) comes from app.css. */
  .summary {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: var(--sp-4);
    font-size: 0.85rem;
    color: var(--text-muted);
  }
  .summary .total {
    font-weight: 600;
    color: var(--text);
  }
  .db-list {
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
  }
  .db-row {
    display: flex;
    flex-direction: column;
    gap: var(--sp-1);
  }
  .db-info {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--sp-3);
  }
  .db-name {
    font-weight: 600;
    font-size: 0.9rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .db-size {
    flex: none;
    font-size: 0.82rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--surface-2);
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    border-radius: 3px;
    background: var(--accent);
    min-width: 2px;
    transition: width 0.2s ease;
  }
  .note {
    margin: var(--sp-4) 0 0;
    font-size: 0.75rem;
    color: var(--text-muted);
  }
  .error-banner {
    background: var(--danger-bg);
    color: var(--danger);
    padding: var(--sp-2) var(--sp-3);
    border-radius: var(--radius-sm);
    font-size: 0.82rem;
    margin-bottom: var(--sp-3);
  }
  .loading,
  .empty {
    color: var(--text-muted);
    font-size: 0.85rem;
  }
</style>
