<script lang="ts">
  import InstanceRow from "$lib/components/InstanceRow.svelte";
  import CreateInstanceForm from "$lib/components/CreateInstanceForm.svelte";
  import Modal from "$lib/components/Modal.svelte";
  import type { Instance, ServiceInfo, ConfigFieldInfo } from "$lib/types";
  import { onMount } from "svelte";
  import { invoke } from '@tauri-apps/api/core';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import ExtensionsModal from '$lib/components/ExtensionsModal.svelte';
  import DatabasesModal from '$lib/components/DatabasesModal.svelte';
  import {
    Play, Square, Settings, Database, Puzzle, FileText, CodeXml, Info, Trash2,
    RefreshCw, Layers, Import, ChevronRight, GripVertical, CircleAlert, Upload, X,
    Check, Copy,
  } from '@lucide/svelte';

  interface Stack {
    id: string;
    name: string;
    description: string | null;
    created_at: string;
    updated_at: string;
  }

  interface BinaryStatus {
    service_type: string;
    installed: boolean;
    version: string | null;
    path: string | null;
  }

  interface DomainInfo {
    id: string;
    subdomain: string;
    target_type: string;
    target_value: string;
    ssl_enabled: boolean;
    full_domain: string;
  }

  let {
    instances = [],
    stacks = [],
    serviceTypes = [],
    binaryStatuses = [],
    installedVersions = {},
    loading = false,
    resolverInstalled = false,
    actionLoading = {},
    tld = "burd",
    // Event handlers
    onStart,
    onStop,
    onRestart,
    onDelete,
    onCreate,
    onViewLogs,
    onViewEnv,
    onViewInfo,
    onOpenSettings,
    onOpenService,
    onOpenDomain,
    onServiceTypeChange,
    onRefresh,
    onNavigateToServices,
    // Stack event handlers
    onCreateStack,
    onDeleteStack,
    onExportStack,
    onImportStack,
    onStartStack,
    onStopStack,
  }: {
    instances: Instance[];
    stacks: Stack[];
    serviceTypes: ServiceInfo[];
    binaryStatuses: BinaryStatus[];
    installedVersions: Record<string, string[]>;
    loading: boolean;
    resolverInstalled: boolean;
    actionLoading: Record<string, boolean>;
    tld?: string;
    onStart: (id: string) => void;
    onStop: (id: string) => void;
    onRestart: (id: string) => void;
    onDelete: (id: string, name: string) => void;
    onCreate: (name: string, port: number, serviceType: string, version: string, config: Record<string, string>, customDomain?: string | null) => void;
    onViewLogs: (id: string, name: string) => void;
    onViewEnv: (id: string, name: string, serviceType: string) => void;
    onViewInfo: (id: string, name: string, serviceType: string) => void;
    onOpenSettings: (instance: Instance) => void;
    onOpenService: (port: number, serviceType: string) => void;
    onOpenDomain: (instance: Instance) => void;
    onServiceTypeChange: (serviceType: string) => { port: number };
    onRefresh: () => void;
    onNavigateToServices: () => void;
    // Stack event handlers
    onCreateStack?: (name: string, description: string | null, instanceIds: string[]) => void;
    onDeleteStack?: (id: string) => void;
    onExportStack?: (id: string) => void;
    onImportStack?: () => void;
    onStartStack?: (id: string) => void;
    onStopStack?: (id: string) => void;
  } = $props();

  async function reorderWithKeyboard(instance: Instance, direction: number) {
    const ids = instances.map(i => i.id);
    const from = ids.indexOf(instance.id);
    const to = Math.max(0, Math.min(ids.length - 1, from + direction));
    if (from === to) return;
    [ids[from], ids[to]] = [ids[to], ids[from]];
    try { await invoke("reorder_instances", { instanceIds: ids }); await onRefresh(); }
    catch (e) { errorMessage = String(e); }
  }

  // Mouse-based drag state (HTML5 DnD doesn't work in Tauri WebView)
  let isDragging = $state(false);
  let draggedInstanceId = $state<string | null>(null);
  let dragOverTarget = $state<string | null>(null);
  let errorMessage = $state<string | null>(null);
  let extensionsInstance = $state<Instance | null>(null);
  let databasesInstance = $state<Instance | null>(null);

  // Per-instance data-directory size (bytes), loaded lazily after the list
  // renders so the walk never blocks the instance list. null = still loading.
  let diskUsage = $state<Record<string, number | null>>({});
  const measuredAt = new Map<string, number>();
  let diskRefresh = $state(0);
  onMount(() => {
    const timer = setInterval(() => { diskRefresh++; }, 60000);
    return () => clearInterval(timer);
  });

  // DB service types that expose a per-database size view (have a db_manager).
  // Refresh sizes when the list changes and once a minute; report failures explicitly.
  $effect(() => {
    void diskRefresh;
    for (const inst of instances) {
      if (Date.now() - (measuredAt.get(inst.id) ?? 0) >= 60000) {
        measuredAt.set(inst.id, Date.now());
        diskUsage[inst.id] = null;
        invoke<number>("get_instance_disk_usage", { instanceId: inst.id })
          .then((bytes) => {
            diskUsage[inst.id] = bytes;
          })
          .catch(() => {
            diskUsage[inst.id] = -1;
          });
      }
    }
  });

  // Reordering state
  let dragOverIndex = $state<number | null>(null);
  let draggedFromIndex = $state<number | null>(null);

  // Stack drag state
  let dragOverInstanceId = $state<string | null>(null);

  // Mouse-based drag-and-drop implementation
  $effect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      // Find which row we're over
      const elements = document.elementsFromPoint(e.clientX, e.clientY);
      const gridRow = elements.find(el => el.classList.contains('grid-row')) as HTMLElement;

      if (gridRow) {
        const rowIndex = gridRow.getAttribute('data-index');
        const instanceId = gridRow.getAttribute('data-instance-id');

        if (instanceId && instanceId !== draggedInstanceId) {
          const instance = instances.find(i => i.id === instanceId);

          if (instance) {
            dragOverInstanceId = instanceId;

            // For standalone instances, also track index for reordering
            if (!instance.stack_id && rowIndex) {
              dragOverIndex = parseInt(rowIndex, 10);
            } else {
              dragOverIndex = null;
            }
          }
        }
      } else {
        dragOverIndex = null;
        dragOverInstanceId = null;
      }
    };

    const handleMouseUp = async (e: MouseEvent) => {
      if (!draggedInstanceId) {
        cleanupDrag();
        return;
      }

      const draggedInstance = instances.find(i => i.id === draggedInstanceId);
      const targetInstance = dragOverInstanceId ? instances.find(i => i.id === dragOverInstanceId) : null;

      if (!draggedInstance) {
        cleanupDrag();
        return;
      }

      try {
        // Determine what operation to perform
        if (targetInstance) {
          const sourceStackId = draggedInstance.stack_id;
          const targetStackId = targetInstance.stack_id;

          if (sourceStackId === targetStackId) {
            // Same context (both standalone or same stack)
            if (!sourceStackId && dragOverIndex !== null && draggedFromIndex !== null && dragOverIndex !== draggedFromIndex) {
              // Reorder within standalone instances
              await handleRowReorder();
            }
            // If same stack, we don't support reordering within stacks yet
          } else {
            // Different contexts - move between stacks or to/from standalone
            if (targetStackId) {
              // Moving TO a stack
              await invoke('move_instance_to_stack', {
                instanceId: draggedInstanceId,
                stackId: targetStackId,
              });
              onRefresh();
            } else {
              // Moving to standalone (removing from stack)
              await invoke('remove_instances_from_stack', {
                instanceIds: [draggedInstanceId],
              });
              onRefresh();
            }
          }
        }
      } catch (error) {
        console.error('❌ Drag operation error:', error);
        errorMessage = `Failed to move instance: ${error}`;
        setTimeout(() => { errorMessage = null; }, 5000);
      }

      cleanupDrag();
    };

    function cleanupDrag() {
      document.body.classList.remove('dragging-instance');
      isDragging = false;
      draggedInstanceId = null;
      dragOverIndex = null;
      draggedFromIndex = null;
      dragOverInstanceId = null;
    }

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
    };
  });

  // Mouse-based drag start (replaces HTML5 dragstart)
  function handleMouseDragStart(event: MouseEvent, instanceId: string) {
    // Only start drag on left mouse button
    if (event.button !== 0) return;

    event.preventDefault(); // Prevent text selection

    isDragging = true;
    draggedInstanceId = instanceId;
    document.body.classList.add('dragging-instance');

    // Track the index for reordering standalone instances
    const standaloneInstances = getStandaloneInstances();
    draggedFromIndex = standaloneInstances.findIndex(i => i.id === instanceId);
  }

  // Handle the actual reorder operation
  async function handleRowReorder() {
    if (draggedFromIndex === null || dragOverIndex === null) return;
    if (draggedFromIndex === dragOverIndex) return;

    try {
      const standaloneInstances = getStandaloneInstances();

      // Create new order by moving the dragged item
      const newOrder = [...standaloneInstances];
      const [draggedInstance] = newOrder.splice(draggedFromIndex, 1);
      newOrder.splice(dragOverIndex, 0, draggedInstance);

      // Get ordered list of IDs
      const orderedIds = newOrder.map(i => i.id);

      // Call backend with the new full order
      await invoke('reorder_instances', {
        instanceIds: orderedIds
      });

      onRefresh();
    } catch (error) {
      console.error('Failed to reorder instances:', error);
      errorMessage = `Failed to reorder: ${error}`;
      setTimeout(() => { errorMessage = null; }, 5000);
    }
  }

  // HTML5 drag-and-drop handlers (fallback, used in template ondragover/ondragleave/ondrop)
  function handleDragOver(e: DragEvent, target: string) {
    e.preventDefault();
    dragOverTarget = target;
  }

  function handleDragLeave(_e: DragEvent) {
    dragOverTarget = null;
  }

  async function handleDrop(e: DragEvent, targetType: string, stackId?: string) {
    e.preventDefault();
    dragOverTarget = null;

    if (!draggedInstanceId) return;

    try {
      if (targetType === 'standalone') {
        await invoke('remove_instances_from_stack', {
          instanceIds: [draggedInstanceId],
        });
        onRefresh();
      } else if (targetType === 'stack' && stackId) {
        await invoke('move_instance_to_stack', {
          instanceId: draggedInstanceId,
          stackId: stackId,
        });
        onRefresh();
      }
    } catch (error) {
      console.error('Drag-drop operation error:', error);
      errorMessage = `Failed to move instance: ${error}`;
      setTimeout(() => { errorMessage = null; }, 5000);
    }
  }

  // Service icons from simple-icons (hardcoded for ESM compatibility)
  // Local form state
  let showCreateForm = $state(false);
  let copiedId = $state<string | null>(null);

  // Stack UI state
  let collapsedStacks = $state<Set<string>>(new Set());
  let selectionMode = $state(false);
  let selectedInstances = $state<Set<string>>(new Set());
  let showCreateStackModal = $state(false);
  let newStackName = $state("");
  let newStackDescription = $state("");

  // Toggle stack collapse
  function toggleStackCollapse(stackId: string) {
    const newSet = new Set(collapsedStacks);
    if (newSet.has(stackId)) {
      newSet.delete(stackId);
    } else {
      newSet.add(stackId);
    }
    collapsedStacks = newSet;
  }

  // Toggle instance selection
  function toggleInstanceSelection(instanceId: string) {
    const newSet = new Set(selectedInstances);
    if (newSet.has(instanceId)) {
      newSet.delete(instanceId);
    } else {
      newSet.add(instanceId);
    }
    selectedInstances = newSet;
  }

  // Get instances for a specific stack
  function getInstancesForStack(stackId: string): Instance[] {
    return instances.filter(i => i.stack_id === stackId);
  }

  // Get standalone instances (not in any stack)
  function getStandaloneInstances(): Instance[] {
    return instances.filter(i => !i.stack_id);
  }

  // Get running count for a stack
  function getStackRunningCount(stackId: string): number {
    return getInstancesForStack(stackId).filter(i => i.running).length;
  }

  // Check if all instances in stack are running
  function isStackFullyRunning(stackId: string): boolean {
    const stackInstances = getInstancesForStack(stackId);
    return stackInstances.length > 0 && stackInstances.every(i => i.running);
  }

  // Cancel selection mode
  function cancelSelection() {
    selectionMode = false;
    selectedInstances = new Set();
  }

  // Open create stack modal
  function openCreateStackModal() {
    if (selectedInstances.size > 0) {
      showCreateStackModal = true;
    }
  }

  // Handle create stack
  function handleCreateStack() {
    if (onCreateStack && newStackName.trim() && selectedInstances.size > 0) {
      onCreateStack(
        newStackName.trim(),
        newStackDescription.trim() || null,
        Array.from(selectedInstances)
      );
      // Reset state
      showCreateStackModal = false;
      newStackName = "";
      newStackDescription = "";
      cancelSelection();
    }
  }

  async function copyToClipboard(text: string, id: string) {
    await navigator.clipboard.writeText(text);
    copiedId = id;
    setTimeout(() => {
      if (copiedId === id) copiedId = null;
    }, 2000);
  }

  async function handleDeleteDomain(fullDomain: string, instanceId: string) {
    try {
      // Confirm deletion
      const confirmed = await confirm(`Are you sure you want to remove domain "${fullDomain}"?\n\nThis will unregister the domain from the proxy and delete it from the configuration.`, {
        title: 'Burd',
        kind: 'warning'
      });

      if (!confirmed) return;

      // Extract subdomain from full domain (remove TLD)
      const subdomain = fullDomain.endsWith(`.${tld}`) ? fullDomain.slice(0, -(tld.length + 1)) : fullDomain;

      // Find the domain ID by subdomain
      const domains = await invoke<DomainInfo[]>("list_domains");
      const domainToDelete = domains.find(d =>
        d.subdomain === subdomain &&
        d.target_type === "instance" &&
        d.target_value === instanceId
      );

      if (!domainToDelete) {
        alert(`Domain "${fullDomain}" not found`);
        return;
      }

      // Delete the domain
      await invoke("delete_domain", { id: domainToDelete.id });

      // Refresh instances list
      await onRefresh();

    } catch (err) {
      alert(`Failed to delete domain: ${err}`);
    }
  }

  function getBinaryStatus(serviceType: string): BinaryStatus | undefined {
    return binaryStatuses.find(b => b.service_type === serviceType);
  }

  function hasWebUI(serviceType: string): boolean {
    return ['meilisearch', 'frankenphp', 'minio'].includes(serviceType.toLowerCase());
  }

  // Get the reason why the start button is disabled (if any)
  function getStartDisabledReason(instance: Instance): string | null {
    if (actionLoading[instance.id]) {
      return "Action in progress...";
    }
    const status = getBinaryStatus(instance.service_type);
    if (!status?.installed) {
      const serviceName = serviceTypes.find(s => s.id === instance.service_type)?.display_name || instance.service_type;
      return `${serviceName} binary not installed. Go to Services section to download it.`;
    }
    return null;
  }

  // Check if start button should be disabled
  function isStartDisabled(instance: Instance): boolean {
    return actionLoading[instance.id] || !getBinaryStatus(instance.service_type)?.installed;
  }
</script>

<div class="instances-section">
  <div class="section-header">
    <div class="title-row">
      <h2>Instances</h2>
      <button class="refresh-btn" onclick={onRefresh} title="Refresh">
        <RefreshCw size={16} strokeWidth={2} />
      </button>
    </div>
    <div class="header-actions">
      {#if instances.length >= 2 && !selectionMode}
        <button
          class="btn secondary small"
          onclick={() => selectionMode = true}
          title="Select instances to group into a stack"
        >
          <Layers size={14} strokeWidth={2} />
          Create Stack
        </button>
      {/if}
      {#if onImportStack}
        <button
          class="btn secondary small"
          onclick={onImportStack}
          title="Import a stack configuration"
        >
          <Import size={14} strokeWidth={2} />
          Import Stack
        </button>
      {/if}
      <button
        class="btn primary small"
        onclick={() => (showCreateForm = !showCreateForm)}

      >
        {showCreateForm ? "Cancel" : "+ New Instance"}
      </button>
    </div>
  </div>

  {#if showCreateForm}
    <CreateInstanceForm {serviceTypes} {installedVersions} {tld} {onCreate} {onServiceTypeChange} {onNavigateToServices} onCreated={() => showCreateForm = false} />
  {/if}

  <!-- Selection mode floating bar -->
  {#if selectionMode}
    <div class="selection-bar">
      <span class="selection-count">{selectedInstances.size} selected</span>
      <div class="selection-actions">
        <button class="btn secondary small" onclick={cancelSelection}>Cancel</button>
        <button
          class="btn primary small"
          onclick={openCreateStackModal}
          disabled={selectedInstances.size === 0}
        >
          Create Stack from Selected
        </button>
      </div>
    </div>
  {/if}

  <!-- Create Stack Modal -->
  {#if showCreateStackModal}
    <Modal onClose={() => showCreateStackModal = false} overlayClass="modal-overlay">
      <div class="modal"  role="document">
        <div class="modal-header">
          <h3 class="modal-title">Create Stack</h3>
          <button aria-label="Close" class="modal-close" onclick={() => showCreateStackModal = false}><X size={16} strokeWidth={2} /></button>
        </div>
        <div class="modal-body">
          <label>
            <span>Stack Name</span>
            <input type="text" bind:value={newStackName} placeholder="My Project Stack" required />
          </label>
          <label>
            <span>Description (optional)</span>
            <textarea bind:value={newStackDescription} placeholder="Backend services for..."></textarea>
          </label>
          <div class="selected-instances-preview">
            <span>Selected instances:</span>
            <ul>
              {#each Array.from(selectedInstances) as instanceId}
                {@const inst = instances.find(i => i.id === instanceId)}
                {#if inst}
                  <li>{inst.name} ({inst.service_type})</li>
                {/if}
              {/each}
            </ul>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn secondary" onclick={() => showCreateStackModal = false}>Cancel</button>
          <button class="btn primary" onclick={handleCreateStack} disabled={!newStackName.trim()}>Create Stack</button>
        </div>
      </div>
    </Modal>
  {/if}

  <!-- Error notification for drag-drop -->
  {#if errorMessage}
    <div class="dnd-error">
      <span>{errorMessage}</span>
      <button aria-label="Close" class="modal-close" onclick={() => errorMessage = null}><X size={16} strokeWidth={2} /></button>
    </div>
  {/if}

  <!-- Standalone drop zone (visible when dragging instance from stack) -->
  {#if isDragging && draggedInstanceId}
    {@const draggedInstance = instances.find(i => i.id === draggedInstanceId)}
    {#if draggedInstance?.stack_id}
      <div
        class="standalone-drop-zone"
        role="region"
        aria-label="Drop zone to move instance to standalone"
        class:drop-target-active={dragOverTarget === 'standalone-zone'}
        ondragover={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, 'standalone-zone');
        }}
        ondragenter={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, 'standalone-zone');
        }}
        ondragleave={handleDragLeave}
        ondrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDrop(e, 'standalone');
        }}
      >
        <div class="drop-zone-hint">
          <Square size={20} strokeWidth={2} />
          Drop here to remove from stack
        </div>
      </div>
    {/if}
  {/if}

    <section class="card">
      {#if loading && instances.length === 0}
        <p class="loading">Loading instances...</p>
      {:else if instances.length === 0}
        <p class="empty">No instances yet. Create one to get started.</p>
      {:else}
        {@const standaloneInstances = getStandaloneInstances()}

        <!-- Main grid -->
        <div class="instances-grid">
          <div class="grid-body">
            {#each standaloneInstances as instance, index (instance.id)}
              {@render instanceRow(instance, true, index)}
            {/each}
          </div>
        </div>
      {/if}
    </section>

    <!-- Stacks -->
    {#each stacks as stack (stack.id)}
    {@const stackInstances = getInstancesForStack(stack.id)}
    {@const runningCount = getStackRunningCount(stack.id)}
    {@const isCollapsed = collapsedStacks.has(stack.id)}
    <div class="stack-group">
      <div
        class="stack-header"
        role="button"
        tabindex="0"
        class:drop-target-active={dragOverTarget === `stack-${stack.id}`}
        ondragover={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, `stack-${stack.id}`);
        }}
        ondragenter={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDragOver(e, `stack-${stack.id}`);
        }}
        ondragleave={(e) => handleDragLeave(e)}
        ondrop={(e) => {
          e.preventDefault();
          e.stopPropagation();
          handleDrop(e, 'stack', stack.id);
        }}
        onclick={(e) => {
          // Only toggle if clicking the header itself, not action buttons
          if (!isDragging && !(e.target as HTMLElement).closest('.stack-actions')) {
            toggleStackCollapse(stack.id);
          }
        }}
        onkeydown={(e) => {
          if ((e.key === 'Enter' || e.key === ' ') && !isDragging && !(e.target as HTMLElement).closest('.stack-actions')) {
            e.preventDefault();
            toggleStackCollapse(stack.id);
          }
        }}
      >
        <div class="stack-toggle">
          <ChevronRight size={14} strokeWidth={2} class={isCollapsed ? '' : 'rotated'} />
        </div>
        <div class="stack-info">
          <span class="stack-name">{stack.name}</span>
          <span class="stack-meta">{stackInstances.length} services · {runningCount} running</span>
          {#if stack.description}
            <span class="stack-description">{stack.description}</span>
          {/if}
        </div>
        <div class="stack-actions" role="group" aria-label="Stack actions">
          {#if onStartStack && !isStackFullyRunning(stack.id)}
            <button class="icon-btn success" onclick={() => onStartStack?.(stack.id)} title="Start All">
              <Play size={14} fill="currentColor" strokeWidth={0} />
            </button>
          {/if}
          {#if onStopStack && runningCount > 0}
            <button class="icon-btn danger" onclick={() => onStopStack?.(stack.id)} title="Stop All">
              <Square size={14} fill="currentColor" strokeWidth={0} />
            </button>
          {/if}
          {#if onExportStack}
            <button class="icon-btn" onclick={() => onExportStack?.(stack.id)} title="Export Stack">
              <Upload size={14} strokeWidth={2} />
            </button>
          {/if}
          {#if onDeleteStack}
            <button class="icon-btn danger" onclick={() => onDeleteStack?.(stack.id)} title="Delete Stack">
              <Trash2 size={14} strokeWidth={2} />
            </button>
          {/if}
        </div>
      </div>
      {#if !isCollapsed}
        <div class="instances-grid stack-instances">
          <div class="grid-body">
            {#each stackInstances as instance, index (instance.id)}
              {@render instanceRow(instance, false, index)}
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/each}
</div>

{#if extensionsInstance}
  <ExtensionsModal
    instanceId={extensionsInstance.id}
    instanceName={extensionsInstance.name}
    onClose={() => (extensionsInstance = null)}
  />
{/if}

{#if databasesInstance}
  <DatabasesModal
    instanceId={databasesInstance.id}
    instanceName={databasesInstance.name}
    onClose={() => (databasesInstance = null)}
  />
{/if}

{#snippet instanceRow(instance: Instance, showCheckbox: boolean = false, index: number = 0)}
  <InstanceRow {instance} {showCheckbox} {index} {selectionMode} {selectedInstances} {draggedInstanceId} {dragOverInstanceId} {resolverInstalled} {copiedId}
    busy={!!actionLoading[instance.id]} diskUsage={diskUsage[instance.id]} disabledReason={getStartDisabledReason(instance)}
    actions={{ onSelect: toggleInstanceSelection, onDragStart: handleMouseDragStart, onReorder: reorderWithKeyboard, onCopy: copyToClipboard,
      onDeleteDomain: handleDeleteDomain, onStart, onStop, onSettings: onOpenSettings, onLogs: onViewLogs, onEnv: onViewEnv,
      onInfo: onViewInfo, onDelete, onDatabases: (instance) => databasesInstance = instance, onExtensions: (instance) => extensionsInstance = instance }} />
{/snippet}

<style>

  .instances-section {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .section-header h2 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .refresh-btn {
    background: none;
    border: none;
    padding: 0.375rem;
    border-radius: 6px;
    cursor: pointer;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05);
    color: var(--text);
  }

  @media (prefers-color-scheme: dark) {
    .refresh-btn {
      color: #98989d;
    }

    .refresh-btn:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }
  }

  .card {
    background: var(--surface);
    border-radius: var(--radius);
    padding: var(--sp-2);
    border: 1px solid var(--border);
    overflow-x: auto;
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

  .instances-grid {
    width: 100%;
    font-size: 0.875rem;
    --grid-columns: 24px 20px minmax(140px, 1.4fr) minmax(100px, 1fr) auto auto auto;
  }

  .grid-body {
    display: contents;
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

  }

  .loading, .empty {
    text-align: center;
    color: var(--text-muted);
    padding: 2rem;
  }

  /* Light mode explicit overrides */
  :global(:root[data-theme="light"]) .card {
    background: var(--surface) !important;
  }

  :global(:root[data-theme="light"]) .refresh-btn {
    color: var(--text-muted) !important;
  }
  :global(:root[data-theme="light"]) .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05) !important;
    color: var(--text) !important;
  }

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

  /* Dark mode explicit overrides */
  :global(:root[data-theme="dark"]) .card {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .refresh-btn {
    color: #98989d !important;
  }
  :global(:root[data-theme="dark"]) .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.1) !important;
    color: #f5f5f7 !important;
  }

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

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) input {
    background: #1c1c1e !important;
    border-color: #3a3a3c !important;
  }

  /* Header actions */
  .header-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
  }

  /* Stack group styles */
  .stack-group {
    margin-bottom: 1rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    background: var(--surface);
  }

  .stack-header {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: var(--surface-2);
    cursor: pointer;
    user-select: none;
    position: relative;
  }

  .stack-header:hover {
    background: #ebebed;
  }

  /* Allow header to receive drag events while children handle clicks */
  .stack-header .stack-toggle,
  .stack-header .stack-info {
    pointer-events: none;
  }

  .stack-header .stack-actions {
    pointer-events: auto;
  }

  .stack-header .stack-actions button {
    pointer-events: auto;
  }

  .stack-toggle {
    color: var(--text-muted);
    transition: transform 0.2s ease;
    display: flex;
    align-items: center;
  }

  .stack-toggle :global(svg.rotated) {
    transform: rotate(90deg);
  }

  .stack-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .stack-name {
    font-weight: 600;
    font-size: 0.9375rem;
  }

  .stack-meta {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .stack-description {
    font-size: 0.8125rem;
    color: var(--text-muted);
    margin-top: 0.125rem;
  }

  .stack-actions {
    display: flex;
    gap: 0.25rem;
  }

  .stack-instances {
    border: none;
  }

  /* Selection bar */
  .selection-bar {
    position: sticky;
    bottom: 1rem;
    left: 0;
    right: 0;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: #1d1d1f;
    color: white;
    border-radius: 10px;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
    z-index: 100;
    margin-top: 1rem;
  }

  .selection-count {
    font-size: 0.875rem;
    font-weight: 500;
  }

  .selection-actions {
    display: flex;
    gap: 0.5rem;
  }

  .selection-bar .btn.secondary {
    background: rgba(255, 255, 255, 0.15);
    color: white;
  }

  .selection-bar .btn.secondary:hover {
    background: rgba(255, 255, 255, 0.25);
  }

  /* Checkbox column */

  /* Modal styles */

  .modal {
    background: var(--surface);
    border-radius: 12px;
    width: 90%;
    max-width: 480px;
    max-height: 90vh;
    overflow-y: auto;
    box-shadow: var(--shadow-modal);
  }

  .modal-body textarea {
    min-height: 80px;
    resize: vertical;
    padding: 0.5rem;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    font-size: 0.875rem;
    font-family: inherit;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 1rem 1.25rem;
    border-top: 1px solid var(--border);
  }

  .selected-instances-preview {
    background: var(--surface-2);
    border-radius: 8px;
    padding: 0.75rem 1rem;
  }

  .selected-instances-preview span {
    font-size: 0.8125rem;
    color: var(--text-muted);
    font-weight: 500;
  }

  .selected-instances-preview ul {
    margin: 0.5rem 0 0 0;
    padding-left: 1.25rem;
  }

  .selected-instances-preview li {
    font-size: 0.875rem;
    color: var(--text);
  }

  :global(:root[data-theme="dark"]) .stack-group {
    border-color: #38383a !important;
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .stack-header {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"]) .stack-header:hover {
    background: #3a3a3c !important;
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

  :global(:root[data-theme="dark"]) .modal-body textarea {
    background: #1c1c1e !important;
    border-color: #3a3a3c !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .selected-instances-preview {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .selected-instances-preview li {
    color: #f5f5f7 !important;
  }

  /* Mapped domains display */

  /* Drag and Drop Styles */

  @media (prefers-color-scheme: dark) {

  }

  .drop-target-active {
    background: rgba(59, 130, 246, 0.1) !important;
    border: 2px dashed #3b82f6 !important;
    outline: 2px solid #3b82f6;
    outline-offset: -2px;
    transition: all 0.2s ease;
  }

  .standalone-drop-zone {
    border: 2px dashed #94a3b8;
    border-radius: 8px;
    padding: 24px;
    margin: 0 0 16px 0;
    text-align: center;
    color: #64748b;
    background: rgba(148, 163, 184, 0.05);
    transition: all 0.2s ease;
  }

  .standalone-drop-zone.drop-target-active {
    background: rgba(16, 185, 129, 0.1);
    border-color: #10b981;
    color: #10b981;
  }

  .drop-zone-hint {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    font-size: 14px;
    font-weight: 500;
  }

  @media (prefers-color-scheme: dark) {

  }

  /* DnD Error notification */
  .dnd-error {
    background: #ff3b30;
    color: white;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    animation: slideIn 0.2s ease;
  }

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
    .dnd-error {
      background: #dc2626;
    }
  }

  /* Drag-and-drop uses mouse events, no special pointer-events needed */
</style>
