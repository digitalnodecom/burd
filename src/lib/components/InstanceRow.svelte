<script lang="ts">
  import type { Instance, InstanceRowActions } from '$lib/types';
  import { getServiceStyle } from '$lib/data/service-styles';
  import { Play, Square, Settings, Database, Puzzle, FileText, CodeXml, Info, Trash2, GripVertical, CircleAlert, X, Check, Copy } from '@lucide/svelte';
  let { instance, index, showCheckbox, selectionMode, selectedInstances, draggedInstanceId, dragOverInstanceId, resolverInstalled, busy, diskUsage, copiedId, disabledReason, actions }: {
    instance: Instance; index: number; showCheckbox: boolean; selectionMode: boolean;
    selectedInstances: Set<string>; draggedInstanceId: string | null; dragOverInstanceId: string | null;
    resolverInstalled: boolean; busy: boolean; diskUsage: number | null | undefined; copiedId: string | null;
    disabledReason: string | null; actions: InstanceRowActions;
  } = $props();
  const isDbInstance = (type: string) => ['postgresql', 'mariadb'].includes(type);
  function formatBytes(n: number | null | undefined): string {
    if (n == null) return "…";
    if (n < 0) return "Unavailable";
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

</script>
            <div
              class="grid-row"
              class:dragging={draggedInstanceId === instance.id}
              class:drag-over={dragOverInstanceId === instance.id && draggedInstanceId !== instance.id}
              data-index={index}
              data-instance-id={instance.id}
            >
              {#if showCheckbox && selectionMode}
                <div class="grid-cell checkbox-col">
                  <input
                    type="checkbox"
                    checked={selectedInstances.has(instance.id)}
                    onchange={() => actions.onSelect(instance.id)}
                  />
                </div>
              {/if}
              {#if !selectionMode}
                <div class="grid-cell drag-col">
                  <div
                    class="drag-handle"
                    role="button"
                    tabindex="0"
                    onmousedown={(e) => actions.onDragStart(e, instance.id)}
                    onkeydown={(event) => { if (event.key === 'ArrowUp' || event.key === 'ArrowDown') { event.preventDefault(); actions.onReorder(instance, event.key === 'ArrowUp' ? -1 : 1); } }}
                    title="Drag to reorder instance"
                    aria-label="Drag to reorder instance"
                  >
                    <GripVertical size={14} strokeWidth={2} />
                  </div>
                </div>
              {/if}
              <div class="grid-cell status-cell">
                {#if instance.running}
                  {#if instance.healthy}
                    <span class="status-dot running" title="Running"></span>
                  {:else if instance.healthy === false}
                    <span class="status-dot unhealthy" title="Unhealthy"></span>
                  {:else}
                    <span class="status-dot starting" title="Starting"></span>
                  {/if}
                {:else}
                  <span class="status-dot stopped" title="Stopped"></span>
                {/if}
              </div>
              <div class="grid-cell name">
                <div class="service-icon" style="--brand-color: {getServiceStyle(instance.service_type).color}" title="{instance.service_type}">
                  <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor">
                    {@html getServiceStyle(instance.service_type).icon}
                  </svg>
                </div>
                <span class="instance-name" title={instance.name}>{instance.name}</span>
              </div>
              <div class="grid-cell domain">
                {#if instance.mapped_domains && instance.mapped_domains.length > 0}
                  <div class="mapped-domains">
                    {#each instance.mapped_domains as domain}
                      <div class="url-cell">
                        {#if instance.running && resolverInstalled}
                          <a
                            href="https://{domain}"
                            target="_blank"
                            rel="noopener noreferrer"
                            class="url-link"
                            title={domain}
                          >
                            {domain}
                          </a>
                        {:else}
                          <span class="domain-inactive">{domain}</span>
                        {/if}
                        <button
                          class="copy-btn"
                          onclick={() => actions.onCopy(domain, `${instance.id}-${domain}`)}
                          title="Copy domain"
                        >
                          {#if copiedId === `${instance.id}-${domain}`}<Check size={13} strokeWidth={2.5} />{:else}<Copy size={13} strokeWidth={2} />{/if}
                        </button>
                        <button
                          class="delete-domain-btn icon-btn danger"
                          onclick={() => actions.onDeleteDomain(domain, instance.id)}
                          title="Remove this domain"
                        >
                          <X size={12} strokeWidth={2} />
                        </button>
                      </div>
                    {/each}
                  </div>
                {:else}
                  <span class="domain-disabled">—</span>
                {/if}
              </div>

              <div class="grid-cell port" title="Port">:{instance.port}</div>
              <div class="grid-cell disk" title="Data directory size on disk">{formatBytes(diskUsage)}</div>
              <div class="grid-cell actions">
                {#if instance.running}
                  <button
                    class="icon-btn danger"
                    onclick={() => actions.onStop(instance.id)}
                    disabled={busy}
                    title="Stop"
                  >
                    <Square size={14} fill="currentColor" strokeWidth={2} />
                  </button>
                {:else}

                  <button
                    class="icon-btn {disabledReason && !busy ? 'warning' : 'success'}"
                    onclick={() => actions.onStart(instance.id)}
                    disabled={!!disabledReason}
                    title={disabledReason || "Start"}
                  >
                    {#if disabledReason && !busy}
                      <CircleAlert size={14} strokeWidth={2} />
                    {:else}
                      <Play size={14} fill="currentColor" strokeWidth={0} />
                    {/if}
                  </button>
                {/if}
                <button
                  class="icon-btn"
                  onclick={() => actions.onSettings(instance)}
                  title="Settings"
                >
                  <Settings size={14} strokeWidth={2} />
                </button>
                {#if isDbInstance(instance.service_type) && instance.running}
                  <button
                    class="icon-btn"
                    onclick={() => actions.onDatabases(instance)}
                    title="Databases and their sizes"
                  >
                    <Database size={14} strokeWidth={2} />
                  </button>
                {/if}
                {#if instance.service_type === "postgresql" && instance.running}
                  <button
                    class="icon-btn"
                    onclick={() => actions.onExtensions(instance)}
                    title="Extensions (pgvector, pg_partman, …)"
                  >
                    <Puzzle size={14} strokeWidth={2} />
                  </button>
                {/if}
                <details class="more-actions">
                  <summary aria-label="More actions for {instance.name}">More</summary>
                  <div class="action-menu">
                <button
                  class="icon-btn"
                  onclick={() => actions.onLogs(instance.id, instance.name)}
                  title="View Logs"
                >
                  <FileText size={14} strokeWidth={2} /> Logs
                </button>
                <button
                  class="icon-btn"
                  onclick={() => actions.onEnv(instance.id, instance.name, instance.service_type)}
                  title="View ENV Variables"
                >
                  <CodeXml size={14} strokeWidth={2} /> Environment
                </button>
                <button
                  class="icon-btn"
                  onclick={() => actions.onInfo(instance.id, instance.name, instance.service_type)}
                  title="View Information"
                >
                  <Info size={14} strokeWidth={2} /> Information
                </button>
                <button
                  class="icon-btn danger"
                  onclick={() => actions.onDelete(instance.id, instance.name)}
                  disabled={busy}
                  title="Delete"
                >
                  <Trash2 size={14} strokeWidth={2} /> Delete instance
                </button>
                  </div>
                </details>
              </div>
            </div>
<style>
  .more-actions { position: relative; }
  .more-actions summary { cursor: pointer; padding: 6px; font-size: 12px; }
  .action-menu { position: absolute; right: 0; top: 100%; z-index: 20; min-width: 160px; padding: 6px; background: var(--surface); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 4px 16px #0002; }
  .action-menu .icon-btn { display: flex; align-items: center; gap: 8px; width: 100%; text-align: left; }

  @media (prefers-color-scheme: dark) {

  }

  input {
    padding: 0.5rem;
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-sm);
    font-size: 0.875rem;
    background: var(--surface);
    color: var(--text);
  }

  @media (prefers-color-scheme: dark) {
    input {
      background: #1c1c1e;
      border-color: #3a3a3c;
    }
  }

  input:focus {
    outline: none;
    border-color: #007aff;
  }

  .grid-row {
    display: grid;
    grid-template-columns: var(--grid-columns);
    gap: 0;
    transition: transform 0.15s ease, opacity 0.15s ease, box-shadow 0.15s ease;
  }

  .drag-handle {
    cursor: grab;
    user-select: none;
  }

  .drag-handle:active {
    cursor: grabbing;
  }

  :global(body.dragging-instance) {
    cursor: grabbing !important;
    user-select: none;
  }

  .grid-cell {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid #f0f0f0;
    display: flex;
    align-items: center;
    min-height: 36px;
    overflow: hidden;
  }

  .grid-row:last-child .grid-cell {
    border-bottom: none;
  }

  .grid-row:hover {
    background: rgba(0, 0, 0, 0.04);
  }

  @media (prefers-color-scheme: dark) {
    .grid-cell {
      border-bottom-color: #38383a;
    }
    .grid-row:hover {
      background: rgba(255, 255, 255, 0.05);
    }
  }

  .grid-cell.name {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .grid-cell.name .service-icon {
    width: 20px;
    height: 20px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--brand-color) 15%, transparent);
    color: var(--brand-color);
    flex-shrink: 0;
  }

  .grid-cell.name .service-icon :global(svg) {
    width: 14px;
    height: 14px;
  }

  .grid-cell.name .instance-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .grid-cell.domain {
    min-width: 0;
    overflow: hidden;
  }

  .grid-cell.port {
    font-family: monospace;
    white-space: nowrap;
    justify-content: flex-start;
  }

  .grid-cell.disk {
    font-family: monospace;
    color: var(--text-muted);
    white-space: nowrap;
    justify-content: flex-start;
    font-variant-numeric: tabular-nums;
  }

  .url-cell {
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .url-link {
    color: #007aff;
    text-decoration: none;
    font-size: 0.8125rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .url-link:hover {
    text-decoration: underline;
  }

  .copy-btn {
    background: none;
    border: none;
    padding: 0.125rem 0.25rem;
    cursor: pointer;
    color: var(--text-muted);
    font-size: 0.75rem;
    border-radius: 3px;
    transition: all 0.15s ease;
  }

  .copy-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: var(--text);
  }

  @media (prefers-color-scheme: dark) {
    .copy-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
  }

  .delete-domain-btn {
    padding: 0.25rem;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.15s ease;
    background: none;
    border: none;
    cursor: pointer;
    border-radius: 3px;
  }

  .url-cell:hover .delete-domain-btn {
    opacity: 1;
  }

  .delete-domain-btn:hover {
    background: rgba(239, 68, 68, 0.1);
  }

  .delete-domain-btn :global(svg) {
    color: #ef4444;
  }

  .domain-inactive, .domain-disabled {
    color: var(--text-muted);
    font-size: 0.8125rem;
  }

  .grid-cell.actions {
    white-space: nowrap;
    display: flex;
    gap: 0.25rem;
    flex-wrap: nowrap;
    justify-content: flex-start;
  }

  .drag-col { padding-inline: 0; }

  .status-cell {
    padding-inline: 0;
    justify-content: center;
  }

  .status-dot {
    flex: 0 0 10px;
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    cursor: default;
  }

  .status-dot.running { background: #22c55e; box-shadow: 0 0 6px rgba(34, 197, 94, 0.5); }
  .status-dot.stopped { background: #9ca3af; }
  .status-dot.starting { background: #f59e0b; animation: pulse 1.5s ease-in-out infinite; }
  .status-dot.unhealthy { background: #ef4444; animation: pulse 1s ease-in-out infinite; }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  @media (prefers-color-scheme: dark) {

  }

  /* Icon button styles */
  .icon-btn {
    background: none;
    border: none;
    padding: 0.375rem;
    cursor: pointer;
    color: var(--text-muted);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: var(--text);
  }

  .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1);
    color: #dc2626;
  }

  .icon-btn.success {
    color: #22c55e;
  }

  .icon-btn.success:hover {
    background: rgba(34, 197, 94, 0.1);
    color: #16a34a;
  }

  .icon-btn.warning {
    color: #f59e0b;
  }

  .icon-btn.warning:hover {
    background: rgba(245, 158, 11, 0.1);
    color: #d97706;
  }

  .icon-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  @media (prefers-color-scheme: dark) {
    .icon-btn {
      color: #98989d;
    }
    .icon-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
    .icon-btn.danger:hover {
      background: rgba(239, 68, 68, 0.2);
      color: #ef4444;
    }
    .icon-btn.success {
      color: #4ade80;
    }
    .icon-btn.success:hover {
      background: rgba(74, 222, 128, 0.2);
      color: #86efac;
    }
    .icon-btn.warning {
      color: #fbbf24;
    }
    .icon-btn.warning:hover {
      background: rgba(251, 191, 36, 0.2);
      color: #fcd34d;
    }
  }

  /* Light mode explicit overrides */

  :global(:root[data-theme="light"]) .grid-cell {
    border-bottom-color: #f0f0f0 !important;
  }

  :global(:root[data-theme="light"]) .grid-row:hover {
    background: rgba(0, 0, 0, 0.04) !important;
  }

  :global(:root[data-theme="light"]) .status-dot.running { background: #22c55e !important; }
  :global(:root[data-theme="light"]) .status-dot.stopped { background: #9ca3af !important; }
  :global(:root[data-theme="light"]) .status-dot.starting { background: #f59e0b !important; }
  :global(:root[data-theme="light"]) .status-dot.unhealthy { background: #ef4444 !important; }

  :global(:root[data-theme="light"]) .icon-btn {
    color: var(--text-muted) !important;
  }
  :global(:root[data-theme="light"]) .icon-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: var(--text) !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.danger:hover {
    background: rgba(220, 38, 38, 0.1) !important;
    color: #dc2626 !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.success {
    color: #22c55e !important;
  }
  :global(:root[data-theme="light"]) .icon-btn.success:hover {
    background: rgba(34, 197, 94, 0.1) !important;
    color: #16a34a !important;
  }

  :global(:root[data-theme="light"]) .copy-btn {
    color: var(--text-muted) !important;
  }
  :global(:root[data-theme="light"]) .copy-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: var(--text) !important;
  }

  /* Dark mode explicit overrides */

  :global(:root[data-theme="dark"]) .grid-cell {
    border-bottom-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .grid-row:hover {
    background: rgba(255, 255, 255, 0.05) !important;
  }

  :global(:root[data-theme="dark"]) .status-dot.running { background: #22c55e !important; }
  :global(:root[data-theme="dark"]) .status-dot.stopped { background: #6b7280 !important; }
  :global(:root[data-theme="dark"]) .status-dot.starting { background: #f59e0b !important; }
  :global(:root[data-theme="dark"]) .status-dot.unhealthy { background: #ef4444 !important; }

  :global(:root[data-theme="dark"]) .icon-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.danger:hover {
    background: rgba(239, 68, 68, 0.2) !important;
    color: #ef4444 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.success {
    color: #4ade80 !important;
  }
  :global(:root[data-theme="dark"]) .icon-btn.success:hover {
    background: rgba(74, 222, 128, 0.2) !important;
    color: #86efac !important;
  }

  :global(:root[data-theme="dark"]) .copy-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .copy-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) input {
    background: #1c1c1e !important;
    border-color: #3a3a3c !important;
  }

  /* Header actions */

  /* Stack group styles */

  /* Allow header to receive drag events while children handle clicks */

  /* Selection bar */

  /* Checkbox column */
  .checkbox-col {
    width: 32px;
    justify-content: center;
  }

  .checkbox-col input[type="checkbox"] {
    width: 16px;
    height: 16px;
    cursor: pointer;
    accent-color: #007aff;
  }

  /* Modal styles */

  /* Mapped domains display */
  .mapped-domains {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    width: 100%;
    overflow: hidden;
  }

  .mapped-domains .url-cell {
    font-size: 0.8125rem;
  }

  /* Drag and Drop Styles */
  .drag-col {
    width: 32px;
    justify-content: center;
    padding: 0.5rem 0.25rem !important;
  }

  .drag-handle {
    padding: 0.25rem;
    cursor: grab;
    color: var(--text-muted);
    opacity: 0.3;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    transition: all 0.15s ease;
    user-select: none;
  }

  .drag-handle:hover {
    opacity: 0.6;
    background: rgba(0, 0, 0, 0.05);
  }

  .drag-handle:active {
    cursor: grabbing;
  }

  .dragging .drag-handle {
    cursor: grabbing !important;
    opacity: 1;  /* Full opacity to show it's the active handle */
  }

  @media (prefers-color-scheme: dark) {
    .drag-handle:hover {
      background: rgba(255, 255, 255, 0.1);
    }
  }

  .grid-row.dragging {
    opacity: 0.6;  /* Slightly less transparent to remain visible */
    transform: scale(0.98);  /* Slight shrink effect */
    background: rgba(59, 130, 246, 0.08) !important;  /* Light blue tint */
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);  /* Elevation shadow */
    border: 2px dashed #3b82f6 !important;  /* Blue dashed border */
    cursor: grabbing !important;  /* Maintain grabbing cursor */
  }

  @media (prefers-color-scheme: dark) {
    .grid-row.dragging {
      opacity: 0.7;  /* Better visibility in dark mode */
      background: rgba(59, 130, 246, 0.12) !important;
      box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);  /* Stronger shadow for dark */
    }
  }

  .grid-row.drag-over {
    border-top: 3px solid #3b82f6;
    border-bottom: 3px solid #3b82f6;
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
