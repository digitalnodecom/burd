<script lang="ts">
  import { ChevronRight, ExternalLink, Folder, Lock, RefreshCw, Trash2, TriangleAlert } from '@lucide/svelte';
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open, confirm, message } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  interface Props {
    tld: string;
    onRefresh: () => void;
  }

  let { tld, onRefresh }: Props = $props();

  interface ParkedDirectory {
    id: string;
    path: string;
    ssl_enabled: boolean;
    project_count: number;
    conflicts: string[];
    created_at: string;
  }

  interface ParkedProject {
    name: string;
    path: string;
    project_type: string;
    domain: string;
    document_root: string;
    status: string;
    isolated: boolean;
    instance_id: string | null;
  }

  interface SyncResult {
    added: string[];
    removed: string[];
    conflicts: string[];
    unchanged: number;
    errors: string[];
  }

  let parkedDirectories = $state<ParkedDirectory[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let parking = $state(false);
  let refreshing = $state<Record<string, boolean>>({});
  let expandedDirs = $state<Record<string, boolean>>({});
  let projectsCache = $state<Record<string, ParkedProject[]>>({});
  let loadingProjects = $state<Record<string, boolean>>({});

  async function loadParkedDirectories() {
    try {
      loading = true;
      error = null;
      parkedDirectories = await invoke<ParkedDirectory[]>("list_parked_directories");
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function loadProjects(dirId: string) {
    if (projectsCache[dirId] && !refreshing[dirId]) {
      return;
    }
    try {
      loadingProjects = { ...loadingProjects, [dirId]: true };
      const projects = await invoke<ParkedProject[]>("get_parked_projects", { id: dirId });
      projectsCache = { ...projectsCache, [dirId]: projects };
    } catch (e) {
      error = String(e);
    } finally {
      loadingProjects = { ...loadingProjects, [dirId]: false };
    }
  }

  async function toggleExpand(dirId: string) {
    const isExpanded = expandedDirs[dirId];
    expandedDirs = { ...expandedDirs, [dirId]: !isExpanded };
    if (!isExpanded) {
      await loadProjects(dirId);
    }
  }

  async function parkDirectory() {
    try {
      const selected = await open({
        title: "Select Directory to Park",
        directory: true,
        multiple: false,
      });

      if (!selected) return;

      parking = true;
      error = null;

      await invoke<ParkedDirectory>("park_directory", {
        path: selected,
        sslEnabled: true,
      });

      await loadParkedDirectories();
      onRefresh();
    } catch (e) {
      error = String(e);
    } finally {
      parking = false;
    }
  }

  async function unparkDirectory(dir: ParkedDirectory) {
    const confirmed = await confirm(
      `Unpark "${dir.path}"? This will remove all associated domains.`,
      { title: "Unpark Directory", kind: "warning" }
    );
    if (!confirmed) return;

    try {
      error = null;
      await invoke("unpark_directory", { id: dir.id });
      await loadParkedDirectories();
      onRefresh();
    } catch (e) {
      error = String(e);
    }
  }

  async function refreshDirectory(dir: ParkedDirectory) {
    try {
      refreshing = { ...refreshing, [dir.id]: true };
      error = null;
      const result = await invoke<SyncResult>("refresh_parked_directory", { id: dir.id });

      if (result.added.length > 0 || result.removed.length > 0) {
        await message(
          `Added: ${result.added.length}, Removed: ${result.removed.length}, Unchanged: ${result.unchanged}`,
          { title: "Sync Complete", kind: "info" }
        );
      }

      await loadParkedDirectories();
      // Refresh projects cache
      delete projectsCache[dir.id];
      if (expandedDirs[dir.id]) {
        await loadProjects(dir.id);
      }
      onRefresh();
    } catch (e) {
      error = String(e);
    } finally {
      refreshing = { ...refreshing, [dir.id]: false };
    }
  }

  async function toggleSsl(dir: ParkedDirectory) {
    try {
      error = null;
      await invoke("update_parked_directory_ssl", {
        id: dir.id,
        sslEnabled: !dir.ssl_enabled,
      });
      await loadParkedDirectories();
    } catch (e) {
      error = String(e);
    }
  }

  function getProjectTypeLabel(type: string): string {
    switch (type) {
      case "php-laravel":
        return "Laravel";
      case "php":
        return "PHP";
      case "static":
        return "Static";
      default:
        return type;
    }
  }

  function getProjectTypeClass(type: string): string {
    switch (type) {
      case "php-laravel":
        return "badge-laravel";
      case "php":
        return "badge-php";
      case "static":
        return "badge-static";
      default:
        return "badge-unknown";
    }
  }

  function getStatusClass(status: string): string {
    switch (status) {
      case "active":
        return "status-active";
      case "isolated":
        return "status-isolated";
      case "conflict":
        return "status-conflict";
      case "pending":
        return "status-pending";
      default:
        return "";
    }
  }

  function openInBrowser(domain: string) {
    window.open(`http://${domain}`, "_blank");
  }

  onMount(() => {
    loadParkedDirectories();

    // Listen for directory changed events from file watcher
    const unlisten = listen("park:directory-changed", async (event) => {
      const { parked_dir_id } = event.payload as { parked_dir_id: string };
      // Refresh that specific directory
      const dir = parkedDirectories.find(d => d.id === parked_dir_id);
      if (dir) {
        await refreshDirectory(dir);
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  });
</script>

<section>
  <header class="section-header">
    <h2>Parked Directories</h2>
    <button class="btn primary small" onclick={parkDirectory} disabled={parking}>
      {parking ? "Parking..." : "+ Park Directory"}
    </button>
  </header>

  {#if error}
    <div class="error-banner">
      <span>{error}</span>
      <button class="close" onclick={() => (error = null)}>&times;</button>
    </div>
  {/if}

  {#if loading}
    <p class="loading">Loading parked directories...</p>
  {:else if parkedDirectories.length === 0}
    <div class="empty-state">
      <div class="empty-icon">
        <Folder size={48} strokeWidth={2} />
      </div>
      <h3>No Parked Directories</h3>
      <p>Park a directory to automatically create domains for all projects inside it.</p>
      <p class="hint">Each subdirectory becomes accessible at <code>subdirectory.{tld}</code></p>
    </div>
  {:else}
    <div class="parked-list">
      {#each parkedDirectories as dir (dir.id)}
        <div class="parked-item">
          <div class="parked-header" onclick={() => toggleExpand(dir.id)} role="button" tabindex="0" onkeypress={(e) => e.key === 'Enter' && toggleExpand(dir.id)}>
            <div class="parked-main">
              <span class="expand-icon" class:expanded={expandedDirs[dir.id]}>
                <ChevronRight size={16} strokeWidth={2} />
              </span>
              <Folder size={20} strokeWidth={2} />
              <div class="parked-info">
                <span class="parked-path">{dir.path}</span>
                <span class="parked-meta">
                  {dir.project_count} project{dir.project_count !== 1 ? "s" : ""}
                  {#if dir.ssl_enabled}
                    <span class="ssl-badge">SSL</span>
                  {/if}
                  {#if dir.conflicts.length > 0}
                    <span class="conflict-badge" title="Some projects have conflicts">{dir.conflicts.length} conflict{dir.conflicts.length !== 1 ? "s" : ""}</span>
                  {/if}
                </span>
              </div>
            </div>
            <div class="parked-actions" role="group" aria-label="Directory actions" onclick={(e) => e.stopPropagation()}>
              <button class="icon-btn" onclick={() => refreshDirectory(dir)} disabled={refreshing[dir.id]} title="Refresh">
                <RefreshCw size={16} strokeWidth={2} />
              </button>
              <button class="icon-btn" onclick={() => toggleSsl(dir)} title={dir.ssl_enabled ? "Disable SSL" : "Enable SSL"}>
                <Lock size={16} strokeWidth={2} />
              </button>
              <button class="icon-btn danger" onclick={() => unparkDirectory(dir)} title="Unpark">
                <Trash2 size={16} strokeWidth={2} />
              </button>
            </div>
          </div>

          {#if expandedDirs[dir.id]}
            <div class="projects-list">
              {#if loadingProjects[dir.id]}
                <p class="loading-projects">Loading projects...</p>
              {:else if projectsCache[dir.id]?.length === 0}
                <p class="no-projects">No projects found in this directory</p>
              {:else}
                {#each projectsCache[dir.id] || [] as project (project.name)}
                  <div class="project-item {getStatusClass(project.status)}">
                    <div class="project-info">
                      <span class="project-name">{project.name}</span>
                      <span class="project-type {getProjectTypeClass(project.project_type)}">{getProjectTypeLabel(project.project_type)}</span>
                      {#if project.isolated}
                        <span class="isolated-badge">Isolated</span>
                      {/if}
                    </div>
                    <div class="project-domain">
                      {#if project.status === "conflict"}
                        <span class="conflict-warning" title="Domain conflicts with another entry">
                          <TriangleAlert size={14} strokeWidth={2} />
                          Conflict
                        </span>
                      {:else if project.status === "pending"}
                        <span class="pending-badge">Pending</span>
                      {:else}
                        <button class="domain-link" onclick={() => openInBrowser(project.domain)}>
                          {project.domain}
                          <ExternalLink size={12} strokeWidth={2} />
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</section>

<style>
  section {
    max-width: 800px;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.5rem;
  }

  .section-header h2 {
    margin: 0;
    font-size: 1.5rem;
    font-weight: 600;
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

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
  }

  .error-banner {
    background: #fee2e2;
    color: #dc2626;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .error-banner .close {
    background: none;
    border: none;
    font-size: 1.25rem;
    cursor: pointer;
    color: inherit;
  }

  .loading {
    text-align: center;
    color: var(--text-muted);
    padding: 2rem;
  }

  .empty-state {
    text-align: center;
    padding: 3rem 2rem;
    background: var(--surface);
    border-radius: 12px;
    border: 1px solid var(--border);
  }

  .empty-icon {
    color: var(--text-muted);
    margin-bottom: 1rem;
  }

  .empty-state h3 {
    margin: 0 0 0.5rem;
    font-size: 1.125rem;
  }

  .empty-state p {
    margin: 0;
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .empty-state .hint {
    margin-top: 0.5rem;
  }

  .empty-state code {
    background: var(--surface-2);
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
    font-size: 0.8125rem;
  }

  .parked-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .parked-item {
    background: var(--surface);
    border-radius: 12px;
    border: 1px solid var(--border);
    overflow: hidden;
  }

  .parked-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .parked-header:hover {
    background: #f9f9f9;
  }

  .parked-main {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
    min-width: 0;
  }

  .expand-icon {
    color: var(--text-muted);
    transition: transform 0.2s ease;
    display: flex;
  }

  .expand-icon.expanded {
    transform: rotate(90deg);
  }

  .folder-icon {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .parked-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    min-width: 0;
  }

  .parked-path {
    font-weight: 500;
    font-size: 0.9375rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .parked-meta {
    font-size: 0.75rem;
    color: var(--text-muted);
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .ssl-badge {
    background: #16a34a;
    color: white;
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
    font-size: 0.625rem;
    font-weight: 600;
  }

  .conflict-badge {
    background: #f59e0b;
    color: white;
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
    font-size: 0.625rem;
    font-weight: 600;
  }

  .parked-actions {
    display: flex;
    gap: 0.375rem;
  }

  .icon-btn {
    background: var(--surface-2);
    border: none;
    border-radius: 6px;
    padding: 0.5rem;
    cursor: pointer;
    color: var(--text-muted);
    transition: all 0.15s ease;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .icon-btn:hover {
    background: #e5e5e5;
    color: var(--text);
  }

  .icon-btn.danger:hover {
    background: #fee2e2;
    color: #dc2626;
  }

  .icon-btn:disabled {
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

  .projects-list {
    border-top: 1px solid var(--border);
    padding: 0.75rem 1rem;
    background: var(--surface-2);
  }

  .loading-projects, .no-projects {
    text-align: center;
    color: var(--text-muted);
    font-size: 0.875rem;
    padding: 1rem;
    margin: 0;
  }

  .project-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.625rem 0.75rem;
    background: var(--surface);
    border-radius: 8px;
    margin-bottom: 0.5rem;
    border: 1px solid var(--border);
  }

  .project-item:last-child {
    margin-bottom: 0;
  }

  .project-item.status-conflict {
    border-color: #f59e0b;
    background: #fffbeb;
  }

  .project-item.status-pending {
    border-color: #6b7280;
    opacity: 0.7;
  }

  .project-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .project-name {
    font-weight: 500;
    font-size: 0.875rem;
  }

  .project-type {
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .badge-laravel {
    background: #ff2d20;
    color: white;
  }

  .badge-php {
    background: #777bb4;
    color: white;
  }

  .badge-static {
    background: #3b82f6;
    color: white;
  }

  .badge-unknown {
    background: #6b7280;
    color: white;
  }

  .isolated-badge {
    background: #8b5cf6;
    color: white;
    font-size: 0.625rem;
    font-weight: 600;
    padding: 0.125rem 0.375rem;
    border-radius: 4px;
  }

  .project-domain {
    display: flex;
    align-items: center;
  }

  .domain-link {
    background: none;
    border: none;
    color: #007aff;
    font-size: 0.8125rem;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0;
  }

  .domain-link:hover {
    text-decoration: underline;
  }

  .conflict-warning {
    color: #f59e0b;
    font-size: 0.75rem;
    display: flex;
    align-items: center;
    gap: 0.25rem;
  }

  .pending-badge {
    color: #6b7280;
    font-size: 0.75rem;
    font-style: italic;
  }

  /* Dark mode */
  @media (prefers-color-scheme: dark) {
    .empty-state {
      background: #2c2c2e;
      border-color: #38383a;
    }

    .empty-state code {
      background: #1c1c1e;
    }

    .parked-item {
      background: #2c2c2e;
      border-color: #38383a;
    }

    .parked-header:hover {
      background: #3a3a3c;
    }

    .projects-list {
      background: #1c1c1e;
      border-top-color: #38383a;
    }

    .project-item {
      background: #2c2c2e;
      border-color: #38383a;
    }

    .project-item.status-conflict {
      background: #451a03;
      border-color: #b45309;
    }

    .icon-btn {
      background: #3a3a3c;
      color: #98989d;
    }

    .icon-btn:hover {
      background: #48484a;
      color: #f5f5f7;
    }

    .icon-btn.danger:hover {
      background: #450a0a;
      color: #fca5a5;
    }

    .error-banner {
      background: #450a0a;
      color: #fca5a5;
    }
  }

  :global(:root[data-theme="dark"]) .empty-state {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .empty-state code {
    background: #1c1c1e !important;
  }

  :global(:root[data-theme="dark"]) .parked-item {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .parked-header:hover {
    background: #3a3a3c !important;
  }

  :global(:root[data-theme="dark"]) .projects-list {
    background: #1c1c1e !important;
    border-top-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .project-item {
    background: #2c2c2e !important;
    border-color: #38383a !important;
  }

  :global(:root[data-theme="dark"]) .project-item.status-conflict {
    background: #451a03 !important;
    border-color: #b45309 !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn {
    background: #3a3a3c !important;
    color: #98989d !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn:hover {
    background: #48484a !important;
    color: #f5f5f7 !important;
  }

  :global(:root[data-theme="dark"]) .icon-btn.danger:hover {
    background: #450a0a !important;
    color: #fca5a5 !important;
  }

  :global(:root[data-theme="dark"]) .error-banner {
    background: #450a0a !important;
    color: #fca5a5 !important;
  }

  :global(:root[data-theme="light"]) .empty-state {
    background: var(--surface) !important;
    border-color: var(--border) !important;
  }

  :global(:root[data-theme="light"]) .empty-state code {
    background: var(--surface-2) !important;
  }

  :global(:root[data-theme="light"]) .parked-item {
    background: var(--surface) !important;
    border-color: var(--border) !important;
  }

  :global(:root[data-theme="light"]) .projects-list {
    background: var(--surface-2) !important;
    border-top-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"]) .project-item {
    background: var(--surface) !important;
    border-color: var(--border) !important;
  }

  :global(:root[data-theme="light"]) .project-item.status-conflict {
    background: #fffbeb !important;
    border-color: #f59e0b !important;
  }
</style>
