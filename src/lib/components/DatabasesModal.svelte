<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

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
      <h2>Databases</h2>
      <span class="subtitle">{instanceName}</span>
      <button class="close-btn" onclick={onClose} aria-label="Close">✕</button>
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
    width: min(520px, 92vw);
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
  .modal-header h2 {
    font-size: 1.1rem;
    margin: 0;
  }
  .subtitle {
    color: #86868b;
    font-size: 0.85rem;
  }
  .close-btn {
    margin-left: auto;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 1rem;
    opacity: 0.6;
  }
  .close-btn:hover {
    opacity: 1;
  }
  .modal-body {
    padding: 16px 20px;
    overflow-y: auto;
  }
  .summary {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 12px;
    font-size: 0.85rem;
    color: #86868b;
  }
  .summary .total {
    font-weight: 600;
    color: inherit;
  }
  .db-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .db-row {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .db-info {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
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
    color: #86868b;
    font-variant-numeric: tabular-nums;
  }
  .bar {
    height: 6px;
    border-radius: 3px;
    background: color-mix(in srgb, currentColor 8%, transparent);
    overflow: hidden;
  }
  .bar-fill {
    height: 100%;
    border-radius: 3px;
    background: var(--accent, #4f8cff);
    min-width: 2px;
    transition: width 0.2s ease;
  }
  .note {
    margin: 14px 0 0;
    font-size: 0.75rem;
    color: #86868b;
  }
  .error-banner {
    background: #ffebe9;
    color: #b00;
    padding: 8px 10px;
    border-radius: 6px;
    font-size: 0.82rem;
    margin-bottom: 10px;
  }
  .loading,
  .empty {
    color: #86868b;
    font-size: 0.85rem;
  }
</style>
