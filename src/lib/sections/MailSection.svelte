<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { createMailState, type MailMessageSummary } from "$lib/composables/useMail.svelte";

  interface Props {
    onRefresh?: () => void;
  }

  let { onRefresh }: Props = $props();

  const mail = createMailState();
  let searchInput = $state("");
  let selectedIds = $state<Set<string>>(new Set());
  let showDeleteAllConfirm = $state(false);
  let showSmtpConfig = $state(false);
  let pollingInterval: ReturnType<typeof setInterval> | null = null;
  let unlisten: UnlistenFn | null = null;

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (mail.selectedEmail) {
        mail.closeEmail();
      } else if (showSmtpConfig) {
        showSmtpConfig = false;
      } else if (showDeleteAllConfirm) {
        showDeleteAllConfirm = false;
      }
    }
  }

  onMount(() => {
    mail.loadSmtpConfig();
    mail.loadEmails();

    // Listen for real-time new email events from backend WebSocket
    listen("new-email", () => {
      mail.refreshEmails();
    }).then((fn) => {
      unlisten = fn;
    });

    // Poll for new emails every 10 seconds as fallback
    pollingInterval = setInterval(() => {
      mail.refreshEmails();
    }, 10000);

    // Listen for ESC key to close modals
    window.addEventListener("keydown", handleKeydown);

    return () => {
      if (pollingInterval) clearInterval(pollingInterval);
      if (unlisten) unlisten();
      window.removeEventListener("keydown", handleKeydown);
    };
  });

  function handleSearch() {
    mail.searchEmails(searchInput);
  }

  function clearSearch() {
    searchInput = "";
    mail.loadEmails();
  }

  function toggleSelect(id: string) {
    if (selectedIds.has(id)) {
      selectedIds.delete(id);
    } else {
      selectedIds.add(id);
    }
    selectedIds = new Set(selectedIds);
  }

  function selectAll() {
    if (selectedIds.size === mail.emails.length) {
      selectedIds = new Set();
    } else {
      selectedIds = new Set(mail.emails.map((e) => e.id));
    }
  }

  async function deleteSelected() {
    const ids = Array.from(selectedIds);
    await mail.deleteSelected(ids);
    selectedIds = new Set();
  }

  async function markSelectedRead(read: boolean) {
    const ids = Array.from(selectedIds);
    await mail.markRead(ids, read);
  }

  function formatDate(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const mins = Math.floor(diff / 60000);
    const hours = Math.floor(diff / 3600000);
    const days = Math.floor(diff / 86400000);

    if (mins < 1) return "Just now";
    if (mins < 60) return `${mins}m ago`;
    if (hours < 24) return `${hours}h ago`;
    if (days < 7) return `${days}d ago`;
    return date.toLocaleDateString();
  }

  function formatAddress(addr: { name: string; address: string }): string {
    return addr.name || addr.address;
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function copyToClipboard(text: string) {
    navigator.clipboard.writeText(text);
  }

  function getLaravelConfig(): string {
    if (!mail.smtpConfig) return "";
    return `MAIL_MAILER=smtp
MAIL_HOST=${mail.smtpConfig.host}
MAIL_PORT=${mail.smtpConfig.port}
MAIL_USERNAME=null
MAIL_PASSWORD=null
MAIL_ENCRYPTION=null
MAIL_FROM_ADDRESS="hello@example.com"
MAIL_FROM_NAME="\${APP_NAME}"`;
  }
</script>

<div class="mail-section">
  <div class="section-header">
    <div class="title-row">
      <h2>Mail</h2>
      <button class="refresh-btn" onclick={() => mail.refreshEmails()} disabled={mail.loading} title="Refresh">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class:spinning={mail.loading}>
          <path d="M21 2v6h-6"></path>
          <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
          <path d="M3 22v-6h6"></path>
          <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
        </svg>
      </button>
      {#if mail.unreadCount > 0}
        <span class="unread-badge">{mail.unreadCount}</span>
      {/if}
    </div>
    <div class="header-actions">
      <form class="search-form" onsubmit={(e) => { e.preventDefault(); handleSearch(); }}>
        <input
          type="text"
          placeholder="Search emails..."
          bind:value={searchInput}
          class="search-input"
        />
        {#if searchInput}
          <button type="button" class="clear-search-btn" onclick={clearSearch} aria-label="Clear search">
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        {/if}
        <button type="submit" class="search-btn" aria-label="Search">
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
        </button>
      </form>
      <button class="btn secondary small" onclick={() => showSmtpConfig = true} title="SMTP Configuration">
        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="3"></circle>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
        </svg>
        SMTP Config
      </button>
    </div>
  </div>

  <!-- Email List -->
  <div class="email-list-container">
    <div class="list-header">
      <div class="list-header-left">
        <span class="count">{mail.totalEmails} message{mail.totalEmails !== 1 ? 's' : ''}</span>
      </div>
      <div class="list-header-actions">
        {#if selectedIds.size > 0}
          <button class="btn-text" onclick={() => markSelectedRead(true)}>Mark Read</button>
          <button class="btn-text" onclick={() => markSelectedRead(false)}>Mark Unread</button>
          <button class="btn-danger-text" onclick={deleteSelected}>Delete ({selectedIds.size})</button>
        {:else if mail.totalEmails > 0}
          <button class="btn-danger-text" onclick={() => showDeleteAllConfirm = true}>Delete All</button>
        {/if}
      </div>
    </div>

    {#if mail.loading && mail.emails.length === 0}
      <div class="loading-state">
        <div class="spinner"></div>
        <span>Loading emails...</span>
      </div>
    {:else if mail.error}
      <div class="error-state">
        <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="12" y1="8" x2="12" y2="12"></line>
          <line x1="12" y1="16" x2="12.01" y2="16"></line>
        </svg>
        <span>{mail.error}</span>
        <button class="btn-secondary" onclick={() => mail.loadEmails()}>Retry</button>
      </div>
    {:else if mail.emails.length === 0}
      <div class="empty-state">
        <svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"></path>
        </svg>
        <span>No emails yet</span>
        <p class="empty-hint">Emails sent to port {mail.smtpConfig?.port || 1025} will appear here</p>
      </div>
    {:else}
      <div class="email-list">
        <div class="select-all-row">
          <label class="checkbox-label">
            <input
              type="checkbox"
              checked={selectedIds.size === mail.emails.length && mail.emails.length > 0}
              onchange={selectAll}
            />
            <span>Select all</span>
          </label>
        </div>
        {#each mail.emails as email}
          <button
            class="email-row"
            class:unread={!email.read}
            onclick={() => mail.viewEmail(email.id)}
          >
            <label class="checkbox-cell" role="none" onclick={(e) => e.stopPropagation()}>
              <input
                type="checkbox"
                checked={selectedIds.has(email.id)}
                onchange={() => toggleSelect(email.id)}
              />
            </label>
            <div class="email-content">
              <div class="email-header-row">
                <span class="from">{formatAddress(email.from)}</span>
                <span class="date">{formatDate(email.created)}</span>
              </div>
              <div class="subject">{email.subject || "(No subject)"}</div>
              <div class="snippet">{email.snippet}</div>
            </div>
            {#if email.attachments > 0}
              <div class="attachment-indicator" title="{email.attachments} attachment{email.attachments !== 1 ? 's' : ''}">
                <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"></path>
                </svg>
              </div>
            {/if}
          </button>
        {/each}
      </div>

      <!-- Pagination -->
      {#if mail.totalEmails > mail.pageSize}
        <div class="pagination">
          <button
            class="btn-secondary"
            onclick={() => mail.prevPage()}
            disabled={!mail.hasPrevPage}
          >
            Previous
          </button>
          <span class="page-info">
            {mail.currentPage * mail.pageSize + 1} - {Math.min((mail.currentPage + 1) * mail.pageSize, mail.totalEmails)} of {mail.totalEmails}
          </span>
          <button
            class="btn-secondary"
            onclick={() => mail.nextPage()}
            disabled={!mail.hasNextPage}
          >
            Next
          </button>
        </div>
      {/if}
    {/if}
  </div>

  <!-- Email Detail Modal -->
  {#if mail.selectedEmail}
    <div class="modal-overlay" onclick={() => mail.closeEmail()} onkeydown={(e) => e.key === 'Escape' && mail.closeEmail()} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal email-modal" onclick={(e) => e.stopPropagation()} role="document">
        <div class="modal-header">
          <button class="back-btn" onclick={() => mail.closeEmail()} title="Back to inbox">
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M19 12H5M12 19l-7-7 7-7"/>
            </svg>
          </button>
          <h3>{mail.selectedEmail.subject || "(No subject)"}</h3>
          <button class="close-btn" onclick={() => mail.closeEmail()} title="Close">
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
        <div class="email-meta">
          <div class="meta-row">
            <span class="meta-label">From:</span>
            <span class="meta-value">{mail.selectedEmail.from.name} &lt;{mail.selectedEmail.from.address}&gt;</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">To:</span>
            <span class="meta-value">
              {mail.selectedEmail.to.map(t => t.name ? `${t.name} <${t.address}>` : t.address).join(", ")}
            </span>
          </div>
          {#if mail.selectedEmail.cc.length > 0}
            <div class="meta-row">
              <span class="meta-label">Cc:</span>
              <span class="meta-value">
                {mail.selectedEmail.cc.map(t => t.name ? `${t.name} <${t.address}>` : t.address).join(", ")}
              </span>
            </div>
          {/if}
          <div class="meta-row">
            <span class="meta-label">Date:</span>
            <span class="meta-value">{new Date(mail.selectedEmail.date).toLocaleString()}</span>
          </div>
          <div class="meta-row">
            <span class="meta-label">Size:</span>
            <span class="meta-value">{formatSize(mail.selectedEmail.size)}</span>
          </div>
        </div>

        {#if mail.selectedEmail.attachments.length > 0}
          <div class="attachments">
            <span class="attachments-label">Attachments:</span>
            {#each mail.selectedEmail.attachments as att}
              <span class="attachment-tag">
                <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M21.44 11.05l-9.19 9.19a6 6 0 0 1-8.49-8.49l9.19-9.19a4 4 0 0 1 5.66 5.66l-9.2 9.19a2 2 0 0 1-2.83-2.83l8.49-8.48"></path>
                </svg>
                {att.file_name} ({formatSize(att.size)})
              </span>
            {/each}
          </div>
        {/if}

        <div class="email-body">
          {#if mail.selectedEmail.html}
            <iframe
              srcdoc={mail.selectedEmail.html}
              sandbox="allow-same-origin"
              title="Email content"
            ></iframe>
          {:else}
            <pre class="text-body">{mail.selectedEmail.text}</pre>
          {/if}
        </div>

        <div class="modal-actions">
          <button class="btn-danger" onclick={() => { mail.deleteEmail(mail.selectedEmail!.id); }}>
            <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
            Delete
          </button>
          <button class="btn-secondary" onclick={() => mail.closeEmail()}>Close</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Delete All Confirmation -->
  {#if showDeleteAllConfirm}
    <div class="modal-overlay" onclick={() => showDeleteAllConfirm = false} onkeydown={(e) => e.key === 'Escape' && (showDeleteAllConfirm = false)} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal confirm-modal" onclick={(e) => e.stopPropagation()} role="document">
        <h3>Delete All Emails?</h3>
        <p>This will permanently delete all {mail.totalEmails} emails. This action cannot be undone.</p>
        <div class="modal-actions">
          <button class="btn-danger" onclick={() => { mail.deleteAll(); showDeleteAllConfirm = false; }}>
            Delete All
          </button>
          <button class="btn-secondary" onclick={() => showDeleteAllConfirm = false}>Cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <!-- SMTP Config Modal -->
  {#if showSmtpConfig && mail.smtpConfig}
    <div class="modal-overlay" onclick={() => showSmtpConfig = false} onkeydown={(e) => e.key === 'Escape' && (showSmtpConfig = false)} role="dialog" aria-modal="true" tabindex="-1">
      <div class="modal config-modal" onclick={(e) => e.stopPropagation()} role="document">
        <div class="modal-header">
          <h3>SMTP Configuration</h3>
          <button class="close-btn" onclick={() => showSmtpConfig = false} title="Close">
            <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
        <div class="config-body">
          <p class="config-hint">Use these settings to send emails to Mailpit from your application.</p>
          <div class="config-grid">
            <div class="config-item">
              <span class="config-label">Host</span>
              <code class="config-value">{mail.smtpConfig.host}</code>
            </div>
            <div class="config-item">
              <span class="config-label">SMTP Port</span>
              <code class="config-value">{mail.smtpConfig.port}</code>
            </div>
            <div class="config-item">
              <span class="config-label">HTTP Port</span>
              <code class="config-value">{mail.smtpConfig.http_port}</code>
            </div>
          </div>
          <div class="config-actions">
            <button class="btn-secondary" onclick={() => copyToClipboard(`${mail.smtpConfig?.host}:${mail.smtpConfig?.port}`)}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
              </svg>
              Copy Host:Port
            </button>
            <button class="btn-secondary" onclick={() => copyToClipboard(getLaravelConfig())}>
              <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
              </svg>
              Copy Laravel .env
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .mail-section {
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

  .title-row .unread-badge {
    background: #007aff;
    color: white;
    font-size: 12px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 10px;
    min-width: 20px;
    text-align: center;
  }

  .header-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .search-form {
    display: flex;
    align-items: center;
    background: #f5f5f7;
    border-radius: 8px;
    padding: 0 8px;
  }

  .search-input {
    border: none;
    background: transparent;
    padding: 8px;
    font-size: 14px;
    width: 180px;
    outline: none;
  }

  .search-btn, .clear-search-btn {
    background: transparent;
    border: none;
    padding: 4px;
    cursor: pointer;
    color: #86868b;
    display: flex;
    align-items: center;
  }

  .search-btn:hover, .clear-search-btn:hover {
    color: #1d1d1f;
  }

  .refresh-btn {
    background: none;
    border: none;
    padding: 0.375rem;
    border-radius: 6px;
    cursor: pointer;
    color: #86868b;
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

  /* Button styles matching Domains */
  .btn {
    display: inline-flex;
    align-items: center;
    gap: 0.375rem;
    padding: 0.5rem 1rem;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    border: none;
    transition: all 0.2s ease;
  }

  .btn.secondary {
    background: #e5e5e5;
    color: #1d1d1f;
  }

  .btn.secondary:hover {
    background: #d1d1d6;
  }

  .btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
  }

  /* Config Modal */
  .config-modal {
    width: 400px;
    max-width: 90vw;
  }

  .config-body {
    padding: 20px;
  }

  .config-hint {
    margin: 0 0 16px 0;
    font-size: 13px;
    color: #636366;
  }

  .config-grid {
    display: grid;
    gap: 12px;
    margin-bottom: 20px;
  }

  .config-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 12px;
    background: #f5f5f7;
    border-radius: 8px;
  }

  .config-label {
    font-size: 13px;
    color: #636366;
  }

  .config-value {
    font-size: 13px;
    font-family: monospace;
    background: #fff;
    padding: 4px 8px;
    border-radius: 4px;
  }

  .config-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  /* Email List */
  .email-list-container {
    background: #fff;
    border: 1px solid #e5e5e5;
    border-radius: 12px;
    overflow: hidden;
  }

  .list-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px;
    border-bottom: 1px solid #e5e5e5;
  }

  .list-header-left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .count {
    font-size: 13px;
    color: #86868b;
  }

  .unread-badge {
    background: #007aff;
    color: white;
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 10px;
    margin-left: 8px;
  }

  .list-header-actions {
    display: flex;
    gap: 8px;
  }

  .select-all-row {
    padding: 8px 16px;
    border-bottom: 1px solid #e5e5e5;
    background: #fafafa;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: #636366;
    cursor: pointer;
  }

  .email-list {
    max-height: 500px;
    overflow-y: auto;
  }

  .email-row {
    display: flex;
    align-items: flex-start;
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

  .email-row:hover {
    background: #f5f5f7;
  }

  .email-row:last-child {
    border-bottom: none;
  }

  .email-row.unread {
    background: #f0f7ff;
  }

  .email-row.unread:hover {
    background: #e5f1ff;
  }

  .email-row.unread .from,
  .email-row.unread .subject {
    font-weight: 600;
  }

  .checkbox-cell {
    padding-top: 4px;
  }

  .email-content {
    flex: 1;
    min-width: 0;
  }

  .email-header-row {
    display: flex;
    justify-content: space-between;
    margin-bottom: 4px;
  }

  .from {
    font-size: 14px;
    color: #1d1d1f;
  }

  .date {
    font-size: 12px;
    color: #86868b;
  }

  .subject {
    font-size: 14px;
    color: #1d1d1f;
    margin-bottom: 4px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .snippet {
    font-size: 13px;
    color: #86868b;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .attachment-indicator {
    color: #86868b;
    padding-top: 4px;
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
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid #e5e5e5;
    border-top-color: #007aff;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  /* Pagination */
  .pagination {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 16px;
    padding: 16px;
    border-top: 1px solid #e5e5e5;
  }

  .page-info {
    font-size: 13px;
    color: #636366;
  }

  /* Buttons */
  .btn-secondary {
    background: #e5e5e5;
    border: none;
    border-radius: 6px;
    padding: 6px 12px;
    font-size: 13px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 6px;
    color: #1d1d1f;
    transition: all 0.15s ease;
  }

  .btn-secondary:hover {
    background: #d1d1d6;
  }

  .btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .btn-text {
    background: transparent;
    border: none;
    padding: 6px 12px;
    font-size: 13px;
    cursor: pointer;
    color: #007aff;
  }

  .btn-text:hover {
    text-decoration: underline;
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
    display: flex;
    align-items: center;
    gap: 6px;
    color: white;
  }

  .btn-danger:hover {
    background: #e0352b;
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
    max-height: 90vh;
    display: flex;
    flex-direction: column;
  }

  .email-modal {
    width: 700px;
    max-width: 90vw;
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

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
    border-bottom: 1px solid #e5e5e5;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
  }

  .close-btn, .back-btn {
    background: transparent;
    border: none;
    padding: 4px;
    cursor: pointer;
    color: #86868b;
    display: flex;
    align-items: center;
  }

  .close-btn:hover, .back-btn:hover {
    color: #1d1d1f;
  }

  .back-btn {
    margin-right: 12px;
  }

  .email-meta {
    padding: 16px 20px;
    background: #f5f5f7;
    border-bottom: 1px solid #e5e5e5;
  }

  .meta-row {
    display: flex;
    gap: 8px;
    margin-bottom: 4px;
    font-size: 13px;
  }

  .meta-row:last-child {
    margin-bottom: 0;
  }

  .meta-label {
    color: #86868b;
    min-width: 50px;
  }

  .meta-value {
    color: #1d1d1f;
  }

  .attachments {
    padding: 12px 20px;
    background: #fafafa;
    border-bottom: 1px solid #e5e5e5;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .attachments-label {
    font-size: 13px;
    color: #86868b;
  }

  .attachment-tag {
    display: flex;
    align-items: center;
    gap: 4px;
    background: #fff;
    border: 1px solid #e5e5e5;
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 12px;
  }

  .email-body {
    flex: 1;
    overflow: auto;
    padding: 0;
    min-height: 300px;
  }

  .email-body iframe {
    width: 100%;
    height: 400px;
    border: none;
  }

  .text-body {
    padding: 20px;
    margin: 0;
    font-family: inherit;
    font-size: 14px;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 16px 20px;
    border-top: 1px solid #e5e5e5;
  }

  /* Dark mode */
  :global(:root[data-theme="dark"]) .search-form {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"]) .search-input {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .search-btn,
  :global(:root[data-theme="dark"]) .clear-search-btn {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .refresh-btn {
    border-color: #38383a;
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .refresh-btn:hover {
    background: #2c2c2e;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .email-list-container {
    background: #1c1c1e;
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .list-header {
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .select-all-row {
    background: #2c2c2e;
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .email-row {
    background: #1c1c1e;
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .email-row:hover {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"]) .email-row.unread {
    background: #1a2a3a;
  }

  :global(:root[data-theme="dark"]) .email-row.unread:hover {
    background: #1f3040;
  }

  :global(:root[data-theme="dark"]) .from,
  :global(:root[data-theme="dark"]) .subject {
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

  :global(:root[data-theme="dark"]) .modal {
    background: #1c1c1e;
  }

  :global(:root[data-theme="dark"]) .modal-header {
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .modal-header h3 {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .email-meta {
    background: #2c2c2e;
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .meta-value {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .attachments {
    background: #2c2c2e;
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .attachment-tag {
    background: #1c1c1e;
    border-color: #38383a;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .text-body {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .modal-actions {
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .pagination {
    border-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .confirm-modal h3 {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .confirm-modal p {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .config-body {
    background: #1c1c1e;
  }

  :global(:root[data-theme="dark"]) .config-hint {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .config-item {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"]) .config-label {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .config-value {
    background: #1c1c1e;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .btn.secondary {
    background: #2c2c2e;
    border-color: #38383a;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .btn.secondary:hover {
    background: #3a3a3c;
  }

  :global(:root[data-theme="dark"]) .title-row .unread-badge {
    background: #0a84ff;
  }
</style>
