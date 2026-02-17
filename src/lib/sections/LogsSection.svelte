<script lang="ts">
  import { invoke, Channel } from "@tauri-apps/api/core";
  import { onMount, onDestroy } from "svelte";

  interface LogEntry {
    id: string;
    source: string;
    instance_id: string | null;
    timestamp: number;
    level: string;
    message: string;
    domain: string | null;
    request_id: string | null;
    method: string | null;
    path: string | null;
    status: number | null;
    duration_ms: number | null;
    context: unknown;
  }

  interface LogSourceInfo {
    id: string;
    name: string;
    log_type: string;
    path: string | null;
    color: string;
  }

  // State
  let logs = $state<LogEntry[]>([]);
  let sources = $state<LogSourceInfo[]>([]);
  let selectedSources = $state<string[]>(["caddy"]);
  let minLevel = $state("INFO");
  let searchTerm = $state("");
  let domainFilter = $state("");
  let autoScroll = $state(true);
  let isPaused = $state(false);
  let isStreaming = $state(false);
  let loading = $state(true);
  let error = $state<string | null>(null);

  const logLevels = ["DEBUG", "INFO", "WARN", "ERROR"];

  // Filtered logs
  let filteredLogs = $derived.by(() => {
    return logs.filter(log => {
      // Filter by source
      if (!selectedSources.includes(log.source)) return false;

      // Filter by log level
      const logLevelIndex = logLevels.indexOf(log.level);
      const minLevelIndex = logLevels.indexOf(minLevel);
      if (logLevelIndex < minLevelIndex) return false;

      // Filter by search term
      if (searchTerm && !log.message.toLowerCase().includes(searchTerm.toLowerCase())) {
        return false;
      }

      // Filter by domain
      if (domainFilter && log.domain !== domainFilter) return false;

      return true;
    });
  });

  // Unique domains from logs
  let uniqueDomains = $derived.by(() => {
    const domains = new Set<string>();
    for (const log of logs) {
      if (log.domain) domains.add(log.domain);
    }
    return Array.from(domains).sort();
  });

  let streamChannel: Channel<LogEntry> | null = null;
  let logsContainer: HTMLDivElement;

  async function loadSources() {
    try {
      sources = await invoke<LogSourceInfo[]>("get_available_log_sources");
    } catch (e) {
      console.error("Failed to load log sources:", e);
    }
  }

  async function loadRecentLogs() {
    loading = true;
    error = null;
    try {
      const recentLogs = await invoke<LogEntry[]>("get_recent_logs", {
        sources: selectedSources,
        limit: 500,
      });
      logs = recentLogs;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function startStreaming() {
    if (isStreaming) return;

    try {
      isStreaming = true;
      streamChannel = new Channel<LogEntry>();

      streamChannel.onmessage = (entry: LogEntry) => {
        if (!isPaused) {
          logs = [entry, ...logs].slice(0, 10000); // Keep last 10k
          if (autoScroll && logsContainer) {
            // Scroll to top since newest are first
            logsContainer.scrollTop = 0;
          }
        }
      };

      // Start streaming (this runs until we cancel)
      invoke("stream_logs", {
        sources: selectedSources,
        onLog: streamChannel,
      }).catch((e: unknown) => {
        console.error("Stream ended:", e);
        isStreaming = false;
      });
    } catch (e) {
      console.error("Failed to start streaming:", e);
      isStreaming = false;
    }
  }

  function stopStreaming() {
    isStreaming = false;
    streamChannel = null;
  }

  function togglePause() {
    isPaused = !isPaused;
  }

  function clearLogs() {
    logs = [];
  }

  function formatTimestamp(ts: number): string {
    return new Date(ts).toLocaleTimeString("en-US", {
      hour12: false,
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      fractionalSecondDigits: 3,
    });
  }

  function getSourceColor(sourceId: string): string {
    const source = sources.find(s => s.id === sourceId);
    return source?.color || "#888";
  }

  function getLevelClass(level: string): string {
    switch (level) {
      case "ERROR": return "level-error";
      case "WARN": return "level-warn";
      case "DEBUG": return "level-debug";
      default: return "level-info";
    }
  }

  function copyRequestId(id: string) {
    navigator.clipboard.writeText(id);
  }

  function exportLogs() {
    const data = filteredLogs.map(log => ({
      timestamp: new Date(log.timestamp).toISOString(),
      source: log.source,
      level: log.level,
      message: log.message,
      domain: log.domain,
      method: log.method,
      path: log.path,
      status: log.status,
    }));

    const json = JSON.stringify(data, null, 2);
    const blob = new Blob([json], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `logs-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  onMount(() => {
    loadSources();
    loadRecentLogs();
    startStreaming();
  });

  onDestroy(() => {
    stopStreaming();
  });
</script>

<div class="logs-section">
  <div class="section-header">
    <div class="title-row">
      <h2>Logs</h2>
      <button class="refresh-btn" onclick={() => loadRecentLogs()} title="Refresh">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M21 2v6h-6"></path>
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
          <path d="M3 22v-6h6"></path>
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
        </svg>
      </button>
    </div>
  </div>

  {#if error}
    <div class="error-banner">
      {error}
      <button onclick={() => error = null}>&times;</button>
    </div>
  {/if}

  <!-- Filters -->
  <div class="filters">
    <div class="filter-row">
      <div class="filter-group sources">
        <span class="filter-label">Sources:</span>
        <div class="source-checkboxes">
          {#each sources as source}
            <label class="source-checkbox">
              <input
                type="checkbox"
                bind:group={selectedSources}
                value={source.id}
              />
              <span class="source-dot" style="background: {source.color}"></span>
              {source.name}
            </label>
          {/each}
        </div>
      </div>

      <div class="filter-group">
        <label for="level-filter">Level:</label>
        <select id="level-filter" bind:value={minLevel}>
          {#each logLevels as level}
            <option value={level}>{level}</option>
          {/each}
        </select>
      </div>

      <div class="filter-group">
        <label for="domain-filter">Domain:</label>
        <select id="domain-filter" bind:value={domainFilter}>
          <option value="">All</option>
          {#each uniqueDomains as domain}
            <option value={domain}>{domain}</option>
          {/each}
        </select>
      </div>

      <div class="filter-group search">
        <label for="search-input">Search:</label>
        <input
          id="search-input"
          type="text"
          placeholder="Filter messages..."
          bind:value={searchTerm}
        />
      </div>
    </div>

    <div class="filter-row controls">
      <label class="checkbox-label">
        <input type="checkbox" bind:checked={autoScroll} />
        Auto-scroll
      </label>
      <button class="btn small" class:active={isPaused} onclick={togglePause}>
        {isPaused ? "Resume" : "Pause"}
      </button>
      <button class="btn small secondary" onclick={exportLogs}>
        Export
      </button>
      <button class="btn small secondary" onclick={clearLogs}>
        Clear
      </button>
      <span class="log-count">
        {filteredLogs.length} of {logs.length} logs
        {#if isStreaming}
          <span class="streaming-indicator" title="Streaming"></span>
        {/if}
      </span>
    </div>
  </div>

  <!-- Log Entries -->
  <div class="logs-container" bind:this={logsContainer}>
    {#if loading && logs.length === 0}
      <div class="loading">Loading logs...</div>
    {:else if filteredLogs.length === 0}
      <div class="empty">
        {#if logs.length === 0}
          No logs yet. Make some requests to see them here.
        {:else}
          No logs match your filters.
        {/if}
      </div>
    {:else}
      {#each filteredLogs as log (log.id)}
        <div class="log-entry {getLevelClass(log.level)}">
          <div class="log-header">
            <span
              class="source-badge"
              style="background: {getSourceColor(log.source)}"
            >
              {log.source.toUpperCase()}
            </span>
            <span class="level">{log.level}</span>
            <span class="timestamp">{formatTimestamp(log.timestamp)}</span>
            {#if log.domain}
              <span class="domain">{log.domain}</span>
            {/if}
          </div>

          <div class="log-message">
            {#if log.method && log.status}
              <span class="http-method">{log.method}</span>
              <span class="http-path">{log.path}</span>
              <span class="http-status" class:success={log.status < 400} class:error={log.status >= 400}>
                {log.status}
              </span>
              {#if log.duration_ms}
                <span class="duration">{log.duration_ms.toFixed(2)}ms</span>
              {/if}
            {:else}
              {log.message}
            {/if}
          </div>

          {#if log.request_id}
            <div class="log-meta">
              <button
                class="request-id"
                onclick={() => copyRequestId(log.request_id!)}
                title="Click to copy"
              >
                Request: {log.request_id.slice(0, 8)}...
              </button>
            </div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>
</div>

<style>
  .logs-section {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 4rem);
  }

  .section-header {
    margin-bottom: 1rem;
  }

  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .title-row h2 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
  }

  .refresh-btn {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.5rem;
    border-radius: 6px;
    color: inherit;
    opacity: 0.7;
  }

  .refresh-btn:hover {
    opacity: 1;
    background: rgba(0, 0, 0, 0.05);
  }

  .error-banner {
    background: #ff3b30;
    color: white;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .error-banner button {
    background: none;
    border: none;
    color: white;
    font-size: 1.25rem;
    cursor: pointer;
  }

  .filters {
    background: #f5f5f7;
    border-radius: 8px;
    padding: 0.75rem 1rem;
    margin-bottom: 1rem;
  }

  @media (prefers-color-scheme: dark) {
    .filters {
      background: #2c2c2e;
    }
  }

  .filter-row {
    display: flex;
    flex-wrap: wrap;
    gap: 1rem;
    align-items: center;
  }

  .filter-row:not(:last-child) {
    margin-bottom: 0.75rem;
  }

  .filter-group {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .filter-group label {
    font-size: 0.85rem;
    font-weight: 500;
    white-space: nowrap;
  }

  .filter-group.sources {
    flex-wrap: wrap;
  }

  .source-checkboxes {
    display: flex;
    gap: 0.75rem;
    flex-wrap: wrap;
  }

  .source-checkbox {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-weight: normal;
    cursor: pointer;
  }

  .source-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
  }

  .filter-group.search {
    flex: 1;
    min-width: 200px;
  }

  .filter-group.search input {
    flex: 1;
  }

  select, input[type="text"] {
    padding: 0.4rem 0.6rem;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 0.85rem;
    background: white;
  }

  @media (prefers-color-scheme: dark) {
    select, input[type="text"] {
      background: #3a3a3c;
      border-color: #48484a;
      color: #f5f5f7;
    }
  }

  .filter-row.controls {
    justify-content: flex-start;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    font-size: 0.85rem;
    cursor: pointer;
  }

  .btn {
    padding: 0.4rem 0.75rem;
    border: none;
    border-radius: 4px;
    font-size: 0.85rem;
    cursor: pointer;
    background: #007aff;
    color: white;
  }

  .btn.secondary {
    background: #8e8e93;
  }

  .btn.small {
    padding: 0.3rem 0.6rem;
  }

  .btn.active {
    background: #ff9500;
  }

  .log-count {
    margin-left: auto;
    font-size: 0.85rem;
    color: #8e8e93;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .streaming-indicator {
    width: 8px;
    height: 8px;
    background: #34c759;
    border-radius: 50%;
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.4; }
  }

  .logs-container {
    flex: 1;
    overflow-y: auto;
    background: #1c1c1e;
    border-radius: 8px;
    padding: 0.5rem;
    font-family: 'SF Mono', 'Monaco', 'Menlo', monospace;
    font-size: 0.8rem;
  }

  .loading, .empty {
    padding: 2rem;
    text-align: center;
    color: #8e8e93;
  }

  .log-entry {
    padding: 0.5rem;
    border-left: 3px solid #48484a;
    margin-bottom: 0.25rem;
    background: rgba(255, 255, 255, 0.02);
    border-radius: 0 4px 4px 0;
  }

  .log-entry.level-error {
    border-left-color: #ff3b30;
    background: rgba(255, 59, 48, 0.1);
  }

  .log-entry.level-warn {
    border-left-color: #ff9500;
    background: rgba(255, 149, 0, 0.1);
  }

  .log-entry.level-info {
    border-left-color: #34c759;
  }

  .log-entry.level-debug {
    border-left-color: #8e8e93;
  }

  .log-header {
    display: flex;
    gap: 0.5rem;
    align-items: center;
    margin-bottom: 0.25rem;
    flex-wrap: wrap;
  }

  .source-badge {
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
    color: white;
    font-weight: 600;
    font-size: 0.7rem;
  }

  .level {
    font-weight: 600;
    min-width: 3.5rem;
    color: #f5f5f7;
  }

  .timestamp {
    color: #8e8e93;
    font-size: 0.75rem;
  }

  .domain {
    color: #007aff;
    font-size: 0.75rem;
  }

  .log-message {
    color: #e5e5e7;
    word-break: break-word;
    line-height: 1.4;
  }

  .http-method {
    font-weight: 600;
    color: #bf5af2;
  }

  .http-path {
    color: #e5e5e7;
  }

  .http-status {
    font-weight: 600;
    padding: 0 0.25rem;
    border-radius: 2px;
  }

  .http-status.success {
    color: #34c759;
  }

  .http-status.error {
    color: #ff3b30;
  }

  .duration {
    color: #8e8e93;
    font-size: 0.75rem;
  }

  .log-meta {
    margin-top: 0.25rem;
  }

  .request-id {
    background: none;
    border: none;
    color: #5ac8fa;
    cursor: pointer;
    padding: 0;
    font-family: inherit;
    font-size: 0.75rem;
    text-decoration: underline;
  }

  .request-id:hover {
    color: #64d2ff;
  }

  @media (prefers-color-scheme: light) {
    .logs-container {
      background: #f2f2f7;
    }

    .log-entry {
      background: white;
    }

    .level, .log-message, .http-path {
      color: #1d1d1f;
    }
  }

  /* Explicit dark theme via data-theme attribute */
  :global(:root[data-theme="dark"]) .filters {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"]) select,
  :global(:root[data-theme="dark"]) input[type="text"] {
    background: #3a3a3c;
    border-color: #48484a;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .logs-container {
    background: #1c1c1e;
  }

  :global(:root[data-theme="dark"]) .refresh-btn:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  /* Explicit light theme via data-theme attribute */
  :global(:root[data-theme="light"]) .filters {
    background: #f5f5f7;
  }

  :global(:root[data-theme="light"]) select,
  :global(:root[data-theme="light"]) input[type="text"] {
    background: white;
    border-color: #ddd;
    color: #1d1d1f;
  }

  :global(:root[data-theme="light"]) .logs-container {
    background: #f2f2f7;
  }

  :global(:root[data-theme="light"]) .log-entry {
    background: white;
  }

  :global(:root[data-theme="light"]) .level,
  :global(:root[data-theme="light"]) .log-message,
  :global(:root[data-theme="light"]) .http-path {
    color: #1d1d1f;
  }

  :global(:root[data-theme="light"]) .refresh-btn:hover {
    background: rgba(0, 0, 0, 0.05);
  }
</style>
