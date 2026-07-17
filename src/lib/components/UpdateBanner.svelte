<script lang="ts">
  import type { createUpdater } from "$lib/composables/useUpdater.svelte";

  let { updater }: { updater: ReturnType<typeof createUpdater> } = $props();
</script>

{#if updater.available}
  <div class="update-banner" role="status">
    <div class="update-info">
      <span class="update-dot"></span>
      {#if updater.downloading}
        <span>Updating to {updater.version}… {updater.progress}%</span>
      {:else}
        <span><strong>Burd {updater.version}</strong> is available</span>
      {/if}
    </div>

    {#if updater.downloading}
      <div class="update-progress">
        <div class="update-progress-bar" style="width: {updater.progress}%"></div>
      </div>
    {:else}
      <div class="update-actions">
        <button class="btn-install" onclick={() => updater.installAndRestart()}>
          Install &amp; Restart
        </button>
        <button class="btn-later" onclick={() => updater.dismiss()}>Later</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .update-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.6rem 1rem;
    background: color-mix(in srgb, var(--accent, #4f8cff) 12%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--accent, #4f8cff) 35%, transparent);
    font-size: 0.9rem;
  }
  .update-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .update-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent, #4f8cff);
    flex: none;
  }
  .update-actions {
    display: flex;
    gap: 0.5rem;
  }
  .btn-install,
  .btn-later {
    border: none;
    border-radius: 6px;
    padding: 0.35rem 0.75rem;
    font-size: 0.85rem;
    cursor: pointer;
  }
  .btn-install {
    background: var(--accent, #4f8cff);
    color: white;
    font-weight: 600;
  }
  .btn-later {
    background: transparent;
    color: inherit;
    opacity: 0.7;
  }
  .btn-later:hover {
    opacity: 1;
  }
  .update-progress {
    flex: 1;
    max-width: 200px;
    height: 6px;
    border-radius: 3px;
    background: color-mix(in srgb, var(--accent, #4f8cff) 20%, transparent);
    overflow: hidden;
  }
  .update-progress-bar {
    height: 100%;
    background: var(--accent, #4f8cff);
    transition: width 0.2s ease;
  }
</style>
