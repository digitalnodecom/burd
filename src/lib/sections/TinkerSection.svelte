<script lang="ts">
  import { onMount } from "svelte";
  import { createTinkerState, type TinkerProject, type TinkerExecution } from "$lib/composables/useTinker.svelte";
  import MonacoEditor from "$lib/components/MonacoEditor.svelte";

  interface Props {
    onRefresh?: () => void;
  }

  let { onRefresh }: Props = $props();

  const tinker = createTinkerState();

  // Example snippets for each project type (no <?php needed - it's auto-prepended)
  const snippets: Record<string, string> = {
    laravel: `// Get all users
$users = \\App\\Models\\User::all();
foreach ($users as $user) {
    echo $user->name . "\\n";
}`,
    wordpress: `// Get recent posts
$posts = get_posts(['numberposts' => 5]);
foreach ($posts as $post) {
    echo $post->post_title . "\\n";
}`,
    bedrock: `// Get recent posts (Bedrock)
$posts = get_posts(['numberposts' => 5]);
foreach ($posts as $post) {
    echo $post->post_title . "\\n";
}`,
    generic: `// Hello World
echo "Hello from Tinker!\\n";
echo "PHP " . phpversion();`,
  };

  function getSnippetForProject(projectType: string | undefined): string {
    return snippets[projectType || 'generic'] || snippets.generic;
  }

  let codeInput = $state(getSnippetForProject(undefined));
  let lastProjectType = $state<string | undefined>(undefined);
  let showClearHistoryConfirm = $state(false);
  let outputMode = $state<"output" | "preview">("output");

  onMount(() => {
    tinker.loadProjects();
    tinker.loadPhpInfo();
    tinker.loadHistory();
  });

  // Update snippet when project type changes
  $effect(() => {
    const currentType = tinker.selectedProject?.project_type;
    if (currentType && currentType !== lastProjectType) {
      // Only update if this is the first project selection or type changed
      // and user hasn't modified the code from the default snippet
      const currentSnippet = getSnippetForProject(lastProjectType);
      if (!lastProjectType || codeInput === currentSnippet) {
        codeInput = getSnippetForProject(currentType);
      }
      lastProjectType = currentType;
    }
  });

  async function runCode() {
    tinker.setCode(codeInput);
    await tinker.execute();
  }

  function handleProjectChange(e: Event) {
    const select = e.target as HTMLSelectElement;
    const project = tinker.projects.find((p) => p.id === select.value);
    if (project) {
      tinker.selectProject(project);
    }
  }

  function restoreHistory(execution: TinkerExecution) {
    codeInput = execution.code;
    tinker.restoreFromHistory(execution);
  }

  function getProjectTypeIcon(type: string): string {
    switch (type) {
      case "laravel":
        return "L"; // Laravel
      case "wordpress":
        return "W"; // WordPress
      case "bedrock":
        return "B"; // Bedrock
      default:
        return "P"; // PHP
    }
  }
</script>

<div class="tinker-section">
  <div class="section-header">
    <div class="header-left">
      <h2>Tinker</h2>
      <div class="header-selectors">
        {#if tinker.hasProjects}
          <div class="selector-group">
            <span class="selector-label">Project:</span>
            <select class="selector-select" onchange={handleProjectChange}>
              {#each tinker.projects as project}
                <option value={project.id} selected={tinker.selectedProject?.id === project.id}>
                  {project.name} ({tinker.getProjectTypeLabel(project.project_type)})
                </option>
              {/each}
            </select>
          </div>
        {/if}
        {#if tinker.phpInfo}
          <div class="selector-group">
            <span class="selector-label">PHP:</span>
            {#if tinker.phpInfo.installed_versions.length > 0}
              <select
                class="selector-select"
                value={tinker.selectedPhpVersion ?? "default"}
                onchange={(e) => {
                  const value = (e.target as HTMLSelectElement).value;
                  tinker.setPhpVersion(value === "default" ? null : value);
                }}
              >
                <option value="default">
                  Default ({tinker.phpInfo.version ?? "none"})
                </option>
                {#each tinker.phpInfo.installed_versions as version}
                  <option value={version}>{version}</option>
                {/each}
              </select>
            {:else}
              <span class="php-badge">
                {tinker.phpInfo.version ?? "Not found"}
              </span>
            {/if}
          </div>
        {/if}
      </div>
    </div>
  </div>

  {#if tinker.loading}
    <div class="loading-state">
      <div class="spinner"></div>
      <span>Loading projects...</span>
    </div>
  {:else if tinker.loadError}
    <div class="error-state">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="12" cy="12" r="10"></circle>
        <line x1="12" y1="8" x2="12" y2="12"></line>
        <line x1="12" y1="16" x2="12.01" y2="16"></line>
      </svg>
      <span>{tinker.loadError}</span>
      <button class="btn-secondary" onclick={() => tinker.loadProjects()}>Retry</button>
    </div>
  {:else if !tinker.hasProjects}
    <div class="empty-state">
      <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <path d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"></path>
      </svg>
      <span>No PHP projects found</span>
      <p class="empty-hint">Create a PHP instance with a document root to start using Tinker</p>
    </div>
  {:else}
    <!-- Code Editor -->
    <div class="code-editor">
      <div class="editor-header">
        <span class="editor-label">Code</span>
        <span class="editor-hint">Press Cmd+Enter to run</span>
      </div>
      <div class="editor-wrapper">
        <MonacoEditor
          value={codeInput}
          language="php-plain"
          theme="auto"
          minHeight={180}
          onchange={(val) => { codeInput = val; }}
          onrun={runCode}
        />
        <button
          class="run-btn"
          onclick={runCode}
          disabled={tinker.executing || !codeInput.trim()}
        >
          {#if tinker.executing}
            <div class="btn-spinner"></div>
          {:else}
            <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="currentColor" stroke="none">
              <polygon points="5 3 19 12 5 21 5 3"></polygon>
            </svg>
            Run
          {/if}
        </button>
      </div>
    </div>

    <!-- Output -->
    <div class="output-section">
      <div class="output-header">
        <div class="output-header-left">
          <span class="output-label">Output</span>
          <div class="output-toggle">
            <button
              class="toggle-btn"
              class:active={outputMode === "output"}
              onclick={() => outputMode = "output"}
            >
              Code
            </button>
            <button
              class="toggle-btn"
              class:active={outputMode === "preview"}
              onclick={() => outputMode = "preview"}
            >
              Preview
            </button>
          </div>
        </div>
        {#if tinker.output || tinker.error}
          <button class="btn-text-small" onclick={() => tinker.clearOutput()}>Clear</button>
        {/if}
      </div>
      <div class="output-content" class:has-error={tinker.error}>
        {#if tinker.executing}
          <div class="output-loading">
            <div class="spinner"></div>
            <span>Executing...</span>
          </div>
        {:else if tinker.output || tinker.error}
          {#if outputMode === "preview"}
            <iframe class="output-preview-frame" srcdoc={tinker.output || tinker.error} sandbox="allow-same-origin" title="Output Preview"></iframe>
          {:else}
            <pre class="output-text" class:has-error={tinker.error && !tinker.output}>{tinker.output || tinker.error}</pre>
          {/if}
        {:else}
          <span class="output-placeholder">Output will appear here</span>
        {/if}
      </div>
    </div>

    <!-- History -->
    <div class="history-section">
      <div class="history-header">
        <h3>History</h3>
        {#if tinker.history.length > 0}
          <button class="btn-danger-text" onclick={() => showClearHistoryConfirm = true}>
            Clear All
          </button>
        {/if}
      </div>
      {#if tinker.history.length === 0}
        <div class="history-empty">
          <span>No history yet</span>
        </div>
      {:else}
        <div class="history-list">
          {#each tinker.history as execution}
            <div class="history-item" onclick={() => restoreHistory(execution)} role="button" tabindex="0" onkeydown={(e) => { if (e.key === 'Enter') restoreHistory(execution); }}>
              <div class="history-content">
                <div class="history-header-row">
                  <span class="history-type-badge {tinker.getProjectTypeColor(execution.project_type)}">
                    {getProjectTypeIcon(execution.project_type)}
                  </span>
                  <code class="history-code">{execution.code.slice(0, 60)}{execution.code.length > 60 ? "..." : ""}</code>
                </div>
                <div class="history-meta">
                  <span class="history-time">{tinker.formatTimeAgo(execution.executed_at)}</span>
                  <span class="history-duration">{tinker.formatDuration(execution.duration_ms)}</span>
                  {#if execution.error}
                    <span class="history-error-badge">Error</span>
                  {/if}
                </div>
              </div>
              <button
                class="delete-btn"
                onclick={(e) => { e.stopPropagation(); tinker.deleteHistoryItem(execution.id); }}
                title="Delete"
              >
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <!-- Clear History Confirmation -->
  {#if showClearHistoryConfirm}
    <div class="modal-overlay" onclick={() => showClearHistoryConfirm = false} onkeydown={(e) => e.key === 'Escape' && (showClearHistoryConfirm = false)} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal confirm-modal" onclick={(e) => e.stopPropagation()} role="document">
        <h3>Clear All History?</h3>
        <p>This will permanently delete all {tinker.history.length} history items. This action cannot be undone.</p>
        <div class="modal-actions">
          <button class="btn-danger" onclick={() => { tinker.clearHistory(); showClearHistoryConfirm = false; }}>
            Clear All
          </button>
          <button class="btn-secondary" onclick={() => showClearHistoryConfirm = false}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .tinker-section {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    max-width: 900px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
  }

  .section-header h2 {
    margin: 0;
    font-size: 24px;
    font-weight: 600;
  }

  .header-selectors {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
  }

  .selector-group {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .selector-label {
    font-size: 13px;
    font-weight: 500;
    color: #636366;
  }

  .selector-select {
    padding: 4px 8px;
    border: 1px solid #e5e5e5;
    border-radius: 8px;
    background: #fff;
    font-size: 13px;
    cursor: pointer;
    color: #1d1d1f;
  }

  .selector-select:hover {
    border-color: #8892bf;
  }

  .selector-select:focus {
    border-color: #8892bf;
    outline: none;
    box-shadow: 0 0 0 2px rgba(136, 146, 191, 0.2);
  }

  .php-badge {
    background: #8892bf;
    color: white;
    font-size: 12px;
    padding: 4px 10px;
    border-radius: 12px;
    font-weight: 500;
  }

  .header-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .refresh-btn {
    background: transparent;
    border: 1px solid #e5e5e5;
    border-radius: 8px;
    padding: 8px;
    cursor: pointer;
    color: #636366;
    display: flex;
    align-items: center;
  }

  .refresh-btn:hover {
    background: #f5f5f7;
    color: #1d1d1f;
  }

  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }


  .project-path {
    font-size: 12px;
    color: #86868b;
    font-family: monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 300px;
  }

  /* Code Editor */
  .code-editor {
    background: #f5f5f7;
    border-radius: 12px;
    margin-bottom: 16px;
  }

  .editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .editor-label {
    font-size: 14px;
    font-weight: 600;
    color: #1d1d1f;
  }

  .editor-hint {
    font-size: 12px;
    color: #86868b;
  }

  .code-textarea {
    width: 100%;
    min-height: 150px;
    padding: 12px;
    border: 1px solid #e5e5e5;
    border-radius: 8px;
    background: #fff;
    font-family: 'SF Mono', 'Menlo', 'Monaco', monospace;
    font-size: 13px;
    line-height: 1.5;
    resize: vertical;
    outline: none;
  }

  .code-textarea:focus {
    border-color: #007aff;
    box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.1);
  }

  .editor-wrapper {
    position: relative;
  }

  .run-btn {
    position: absolute;
    bottom: 12px;
    right: 12px;
    background: #007aff;
    border: none;
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 4px;
    color: white;
    transition: all 0.15s ease;
    z-index: 10;
  }

  .run-btn:hover:not(:disabled) {
    background: #0066d6;
  }

  .run-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  /* Output */
  .output-section {
    background: #1d1d1f;
    border-radius: 12px;
    padding: 16px;
    margin-bottom: 24px;
  }

  .output-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }

  .output-header-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .output-label {
    font-size: 14px;
    font-weight: 600;
    color: #98989d;
  }

  .output-toggle {
    display: flex;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 2px;
  }

  .toggle-btn {
    background: transparent;
    border: none;
    padding: 4px 10px;
    font-size: 12px;
    color: #86868b;
    cursor: pointer;
    border-radius: 4px;
    transition: all 0.15s ease;
  }

  .toggle-btn:hover {
    color: #c9d1d9;
  }

  .toggle-btn.active {
    background: rgba(255, 255, 255, 0.15);
    color: #f5f5f7;
  }

  .btn-text-small {
    background: transparent;
    border: none;
    padding: 4px 8px;
    font-size: 12px;
    cursor: pointer;
    color: #86868b;
  }

  .btn-text-small:hover {
    color: #f5f5f7;
  }

  .output-content {
    min-height: 100px;
    font-family: 'SF Mono', 'Menlo', 'Monaco', monospace;
    font-size: 13px;
    line-height: 1.5;
    background: transparent;
  }

  .output-text {
    color: #c9d1d9;
    margin: 0;
    padding: 0;
    white-space: pre-wrap;
    word-break: break-word;
    background: none !important;
    border: none;
  }

  .output-preview-frame {
    width: 100%;
    min-height: 200px;
    border: none;
    border-radius: 6px;
    background: #ffffff;
  }

  .output-error {
    color: #ff6b6b;
    margin: 0 0 12px 0;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .output-placeholder {
    color: #636366;
    font-style: italic;
  }

  .output-loading {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #98989d;
  }

  /* History */
  .history-section {
    background: #fff;
    border: 1px solid #e5e5e5;
    border-radius: 12px;
    overflow: hidden;
  }

  .history-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px;
    border-bottom: 1px solid #e5e5e5;
  }

  .history-header h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .history-empty {
    padding: 32px;
    text-align: center;
    color: #86868b;
    font-size: 14px;
  }

  .history-list {
    max-height: 300px;
    overflow-y: auto;
  }

  .history-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    border: none;
    border-bottom: 1px solid #e5e5e5;
    background: #fff;
    cursor: pointer;
    width: 100%;
    text-align: left;
    transition: background 0.15s ease;
  }

  .history-item:hover {
    background: #f5f5f7;
  }

  .history-item:last-child {
    border-bottom: none;
  }

  .history-content {
    flex: 1;
    min-width: 0;
  }

  .history-header-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .history-type-badge {
    font-size: 10px;
    font-weight: 700;
    padding: 2px 6px;
    border-radius: 3px;
    background: rgba(0, 0, 0, 0.05);
  }

  .history-code {
    font-size: 13px;
    font-family: 'SF Mono', 'Menlo', 'Monaco', monospace;
    color: #1d1d1f;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .history-meta {
    display: flex;
    gap: 12px;
    font-size: 12px;
    color: #86868b;
  }

  .history-error-badge {
    color: #ff3b30;
    font-weight: 500;
  }

  .delete-btn {
    background: transparent;
    border: none;
    padding: 4px;
    cursor: pointer;
    color: #86868b;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .history-item:hover .delete-btn {
    opacity: 1;
  }

  .delete-btn:hover {
    color: #ff3b30;
  }

  /* Buttons */
  .btn-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-top-color: white;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .btn-secondary {
    background: #f5f5f7;
    border: 1px solid #e5e5e5;
    border-radius: 8px;
    padding: 10px 20px;
    font-size: 14px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    color: #1d1d1f;
    transition: all 0.15s ease;
  }

  .btn-secondary:hover {
    background: #e8e8ed;
  }

  .btn-danger-text {
    background: transparent;
    border: none;
    padding: 6px 12px;
    font-size: 13px;
    cursor: pointer;
    color: #ff3b30;
  }

  .btn-danger-text:hover {
    text-decoration: underline;
  }

  .btn-danger {
    background: #ff3b30;
    border: none;
    border-radius: 6px;
    padding: 8px 16px;
    font-size: 13px;
    cursor: pointer;
    color: white;
  }

  .btn-danger:hover {
    background: #e0352b;
  }

  /* States */
  .loading-state, .error-state, .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 48px;
    color: #86868b;
    gap: 12px;
  }

  .empty-state svg {
    opacity: 0.5;
  }

  .empty-hint {
    font-size: 13px;
    margin: 0;
    text-align: center;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid #e5e5e5;
    border-top-color: #007aff;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  /* Project Type Colors */
  .text-red-400 {
    color: #f87171;
  }

  .text-blue-400 {
    color: #60a5fa;
  }

  .text-teal-400 {
    color: #2dd4bf;
  }

  .text-purple-400 {
    color: #a78bfa;
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: #fff;
    border-radius: 12px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
  }

  .confirm-modal {
    width: 400px;
    padding: 24px;
  }

  .confirm-modal h3 {
    margin: 0 0 12px 0;
  }

  .confirm-modal p {
    margin: 0 0 24px 0;
    color: #636366;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  /* Dark mode */
  :global(:root[data-theme="dark"]) .selector-label {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .selector-select {
    background: #2c2c2e;
    border-color: #38383a;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .selector-select:hover {
    border-color: #8892bf;
  }

  :global(:root[data-theme="dark"]) .selector-select:focus {
    border-color: #8892bf;
    box-shadow: 0 0 0 2px rgba(136, 146, 191, 0.3);
  }

  :global(:root[data-theme="dark"]) .code-editor {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"]) .editor-label {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .code-textarea {
    background: #1c1c1e;
    border-color: #38383a;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .code-textarea:focus {
    border-color: #0a84ff;
    box-shadow: 0 0 0 3px rgba(10, 132, 255, 0.2);
  }

  :global(:root[data-theme="dark"]) .output-section {
    background: #1c1c1e;
  }

  :global(:root[data-theme="dark"]) .history-section {
    background: #1c1c1e;
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .history-header {
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .history-item {
    background: #1c1c1e;
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .history-item:hover {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"]) .history-code {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .btn-secondary {
    background: #2c2c2e;
    border-color: #38383a;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .btn-secondary:hover {
    background: #3a3a3c;
  }

  :global(:root[data-theme="dark"]) .refresh-btn {
    border-color: #38383a;
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .refresh-btn:hover {
    background: #2c2c2e;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .modal {
    background: #1c1c1e;
  }

  :global(:root[data-theme="dark"]) .confirm-modal h3 {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .confirm-modal p {
    color: #98989d;
  }
</style>
