<script lang="ts">
  interface Stack {
    id: string;
    name: string;
    description: string | null;
    created_at: string;
    updated_at: string;
  }

  interface Instance {
    id: string;
    name: string;
    service_type: string;
    running: boolean;
    stack_id: string | null;
  }

  let {
    show = false,
    stack = null,
    instances = [],
    onClose,
    onConfirm,
  }: {
    show: boolean;
    stack: Stack | null;
    instances: Instance[];
    onClose: () => void;
    onConfirm: (stackId: string, deleteInstances: boolean) => void;
  } = $props();

  // Form state
  type DeleteOption = "keep" | "delete";
  let deleteOption = $state<DeleteOption>("keep");
  let confirmText = $state("");

  // Derived
  let stackInstances = $derived(
    stack ? instances.filter(i => i.stack_id === stack.id) : []
  );
  let runningCount = $derived(stackInstances.filter(i => i.running).length);
  let requiresConfirmation = $derived(deleteOption === "delete" && stackInstances.length > 0);
  let confirmationValid = $derived(
    !requiresConfirmation || confirmText.toLowerCase() === "delete"
  );

  // Reset state when modal opens
  $effect(() => {
    if (show && stack) {
      deleteOption = "keep";
      confirmText = "";
    }
  });

  function handleClose() {
    onClose();
  }

  function handleConfirm() {
    if (!stack || !confirmationValid) return;
    onConfirm(stack.id, deleteOption === "delete");
  }

  function formatServiceType(type: string): string {
    return type.charAt(0).toUpperCase() + type.slice(1).toLowerCase();
  }
</script>

{#if show && stack}
  <div class="modal-overlay" onclick={handleClose} onkeydown={(e) => e.key === 'Escape' && handleClose()} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Delete Stack</h3>
        <button class="close-btn" onclick={handleClose}>&times;</button>
      </div>

      <div class="modal-body">
        <p class="delete-prompt">
          Delete "<strong>{stack.name}</strong>"?
        </p>

        {#if stackInstances.length > 0}
          <div class="instances-info">
            <p>This stack contains <strong>{stackInstances.length}</strong> instance{stackInstances.length !== 1 ? "s" : ""}:</p>
            <ul class="instance-list">
              {#each stackInstances as instance}
                <li>
                  <span class="instance-name">{instance.name}</span>
                  <span class="instance-type">({formatServiceType(instance.service_type)})</span>
                  {#if instance.running}
                    <span class="running-badge">Running</span>
                  {/if}
                </li>
              {/each}
            </ul>
          </div>

          <div class="options-section">
            <p class="options-label">What would you like to do with the instances?</p>

            <label class="option-card" class:selected={deleteOption === "keep"}>
              <input
                type="radio"
                name="delete-option"
                value="keep"
                bind:group={deleteOption}
              />
              <div class="option-content">
                <span class="option-title">Keep instances</span>
                <span class="option-desc">Move instances to Standalone (recommended)</span>
              </div>
            </label>

            <label class="option-card danger" class:selected={deleteOption === "delete"}>
              <input
                type="radio"
                name="delete-option"
                value="delete"
                bind:group={deleteOption}
              />
              <div class="option-content">
                <span class="option-title">Delete all instances</span>
                <span class="option-desc">Permanently remove all instances and their data</span>
              </div>
            </label>
          </div>

          {#if deleteOption === "delete"}
            <div class="danger-zone">
              {#if runningCount > 0}
                <div class="warning-banner">
                  <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                    <line x1="12" y1="9" x2="12" y2="13"/>
                    <line x1="12" y1="17" x2="12.01" y2="17"/>
                  </svg>
                  <span><strong>{runningCount}</strong> instance{runningCount !== 1 ? "s are" : " is"} currently running. They will be stopped before deletion.</span>
                </div>
              {/if}

              <div class="confirm-input">
                <label>
                  <span>Type <strong>delete</strong> to confirm:</span>
                  <input
                    type="text"
                    bind:value={confirmText}
                    placeholder="delete"
                    autocomplete="off"
                  />
                </label>
              </div>
            </div>
          {/if}
        {:else}
          <p class="empty-stack-notice">This stack has no instances. It will be removed.</p>
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn secondary" onclick={handleClose}>
          Cancel
        </button>
        <button
          class="btn {deleteOption === 'delete' ? 'danger' : 'primary'}"
          onclick={handleConfirm}
          disabled={!confirmationValid}
        >
          {#if deleteOption === "delete"}
            Delete Stack & Instances
          {:else}
            Delete Stack
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
    max-width: 480px;
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

  .delete-prompt {
    margin: 0 0 1rem 0;
    font-size: 1rem;
  }

  .instances-info {
    margin-bottom: 1.25rem;
  }

  .instances-info p {
    margin: 0 0 0.5rem 0;
    font-size: 0.875rem;
    color: #86868b;
  }

  .instance-list {
    margin: 0;
    padding: 0;
    list-style: none;
    background: #f5f5f7;
    border-radius: 8px;
    padding: 0.5rem;
    max-height: 150px;
    overflow-y: auto;
  }

  .instance-list li {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.375rem 0.5rem;
    font-size: 0.875rem;
  }

  .instance-name {
    font-weight: 500;
  }

  .instance-type {
    color: #86868b;
    font-size: 0.8125rem;
  }

  .running-badge {
    background: #dcfce7;
    color: #16a34a;
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
    text-transform: uppercase;
    margin-left: auto;
  }

  .options-section {
    margin-bottom: 1rem;
  }

  .options-label {
    margin: 0 0 0.75rem 0;
    font-size: 0.875rem;
    font-weight: 500;
  }

  .option-card {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border: 2px solid #e5e5e5;
    border-radius: 8px;
    margin-bottom: 0.5rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .option-card:hover {
    border-color: #d1d1d6;
  }

  .option-card.selected {
    border-color: #007aff;
    background: #f0f7ff;
  }

  .option-card.danger.selected {
    border-color: #dc2626;
    background: #fef2f2;
  }

  .option-card input[type="radio"] {
    margin-top: 0.125rem;
    accent-color: #007aff;
  }

  .option-card.danger input[type="radio"] {
    accent-color: #dc2626;
  }

  .option-content {
    display: flex;
    flex-direction: column;
    gap: 0.125rem;
  }

  .option-title {
    font-weight: 500;
    font-size: 0.875rem;
  }

  .option-desc {
    font-size: 0.75rem;
    color: #86868b;
  }

  .danger-zone {
    margin-top: 1rem;
  }

  .warning-banner {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: #fef3c7;
    border-radius: 8px;
    color: #92400e;
    font-size: 0.8125rem;
    margin-bottom: 1rem;
  }

  .warning-banner svg {
    flex-shrink: 0;
    margin-top: 0.125rem;
  }

  .confirm-input label {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
  }

  .confirm-input span {
    font-size: 0.875rem;
  }

  .confirm-input input {
    padding: 0.5rem 0.75rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
  }

  .confirm-input input:focus {
    outline: none;
    border-color: #dc2626;
  }

  .empty-stack-notice {
    color: #86868b;
    font-size: 0.875rem;
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

  .btn.danger {
    background: #dc2626;
    color: white;
  }

  .btn.danger:hover:not(:disabled) {
    background: #b91c1c;
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

    .instance-list {
      background: #1c1c1e;
    }

    .running-badge {
      background: #14532d;
    }

    .option-card {
      border-color: #38383a;
    }

    .option-card:hover {
      border-color: #48484a;
    }

    .option-card.selected {
      border-color: #0a84ff;
      background: #1c3a5e;
    }

    .option-card.danger.selected {
      border-color: #ef4444;
      background: #3d2020;
    }

    .warning-banner {
      background: #451a03;
      color: #fcd34d;
    }

    .confirm-input input {
      background: #1c1c1e;
      border-color: #38383a;
      color: #f5f5f7;
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

  :global(:root[data-theme="dark"]) .instance-list {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .option-card {
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .option-card.selected {
    border-color: #0a84ff !important;
    background: #1c3a5e !important;
  }

  :global(:root[data-theme="dark"]) .option-card.danger.selected {
    border-color: #ef4444 !important;
    background: #3d2020 !important;
  }

  :global(:root[data-theme="dark"]) .warning-banner {
    background: #451a03 !important;
    color: #fcd34d !important;
  }

  :global(:root[data-theme="dark"]) .confirm-input input {
    background: #1c1c1e !important;
    border-color: #38383a !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #3a3a3c !important;
    color: #f5f5f7 !important;
  }
</style>
