/**
 * Mail state composable for Mailpit integration
 * Handles email list, viewing, search, and actions
 */

import { invoke } from "@tauri-apps/api/core";

// ============================================================================
// Types
// ============================================================================

export interface MailAddress {
  name: string;
  address: string;
}

export interface MailAttachment {
  part_id: string;
  file_name: string;
  content_type: string;
  size: number;
}

export interface MailMessageSummary {
  id: string;
  message_id: string;
  from: MailAddress;
  to: MailAddress[];
  subject: string;
  created: string;
  size: number;
  read: boolean;
  snippet: string;
  attachments: number;
}

export interface MailMessageList {
  total: number;
  unread: number;
  count: number;
  start: number;
  messages: MailMessageSummary[];
}

export interface MailMessageDetail {
  id: string;
  message_id: string;
  from: MailAddress;
  to: MailAddress[];
  cc: MailAddress[];
  bcc: MailAddress[];
  reply_to: MailAddress[];
  subject: string;
  date: string;
  size: number;
  html: string;
  text: string;
  attachments: MailAttachment[];
}

export interface SmtpConfig {
  host: string;
  port: number;
  http_port: number;
}

// ============================================================================
// Class-based state (Svelte 5 recommended pattern for shared reactive state)
// ============================================================================

export class MailState {
  emails = $state<MailMessageSummary[]>([]);
  totalEmails = $state(0);
  unreadCount = $state(0);
  loading = $state(false);
  error = $state<string | null>(null);
  searchQuery = $state("");
  selectedEmail = $state<MailMessageDetail | null>(null);
  selectedEmailLoading = $state(false);
  smtpConfig = $state<SmtpConfig | null>(null);
  currentPage = $state(0);
  pageSize = 25;

  get hasNextPage() {
    return (this.currentPage + 1) * this.pageSize < this.totalEmails;
  }

  get hasPrevPage() {
    return this.currentPage > 0;
  }

  async loadSmtpConfig() {
    try {
      this.smtpConfig = await invoke<SmtpConfig>("get_mailpit_config");
    } catch (e) {
      console.error("Failed to load SMTP config:", e);
      this.smtpConfig = null;
    }
  }

  async loadEmails(page = 0, search = "") {
    // Guard against concurrent requests
    if (this.loading) {
      return;
    }

    this.loading = true;
    this.error = null;
    try {
      const result = await invoke<MailMessageList>("list_emails", {
        start: page * this.pageSize,
        limit: this.pageSize,
        search: search || undefined,
      });
      this.emails = result.messages;
      this.totalEmails = result.total;
      this.unreadCount = result.unread;
      this.currentPage = page;
      this.searchQuery = search;
    } catch (e) {
      console.error("[Mail] loadEmails error", e);
      this.error = e instanceof Error ? e.message : String(e);
      this.emails = [];
    } finally {
      this.loading = false;
    }
  }

  async refreshEmails() {
    await this.loadEmails(this.currentPage, this.searchQuery);
  }

  async searchEmails(query: string) {
    await this.loadEmails(0, query);
  }

  async viewEmail(messageId: string) {
    this.selectedEmailLoading = true;
    try {
      const email = await invoke<MailMessageDetail>("get_email", { messageId });
      this.selectedEmail = email;
      // Mark as read if currently unread in the list
      const idx = this.emails.findIndex((e) => e.id === messageId);
      if (idx !== -1 && !this.emails[idx].read) {
        await this.markRead([messageId], true);
        this.emails[idx].read = true;
        this.unreadCount = Math.max(0, this.unreadCount - 1);
      }
    } catch (e) {
      console.error("Failed to load email:", e);
      this.error = e instanceof Error ? e.message : String(e);
    } finally {
      this.selectedEmailLoading = false;
    }
  }

  closeEmail() {
    this.selectedEmail = null;
  }

  async deleteEmail(messageId: string) {
    try {
      await invoke("delete_emails", { messageIds: [messageId] });
      // Remove from local state
      const idx = this.emails.findIndex((e) => e.id === messageId);
      if (idx !== -1) {
        if (!this.emails[idx].read) {
          this.unreadCount = Math.max(0, this.unreadCount - 1);
        }
        this.emails.splice(idx, 1);
        this.totalEmails--;
      }
      if (this.selectedEmail?.id === messageId) {
        this.selectedEmail = null;
      }
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  async deleteSelected(messageIds: string[]) {
    try {
      await invoke("delete_emails", { messageIds });
      // Update local state
      for (const id of messageIds) {
        const idx = this.emails.findIndex((e) => e.id === id);
        if (idx !== -1) {
          if (!this.emails[idx].read) {
            this.unreadCount = Math.max(0, this.unreadCount - 1);
          }
          this.emails.splice(idx, 1);
          this.totalEmails--;
        }
      }
      if (this.selectedEmail && messageIds.includes(this.selectedEmail.id)) {
        this.selectedEmail = null;
      }
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  async deleteAll() {
    try {
      await invoke("delete_all_emails");
      this.emails = [];
      this.totalEmails = 0;
      this.unreadCount = 0;
      this.selectedEmail = null;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  async markRead(messageIds: string[], read: boolean) {
    try {
      await invoke("mark_emails_read", { messageIds, read });
      // Update local state
      for (const id of messageIds) {
        const idx = this.emails.findIndex((e) => e.id === id);
        if (idx !== -1) {
          const wasRead = this.emails[idx].read;
          this.emails[idx].read = read;
          if (wasRead && !read) {
            this.unreadCount++;
          } else if (!wasRead && read) {
            this.unreadCount = Math.max(0, this.unreadCount - 1);
          }
        }
      }
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
    }
  }

  nextPage() {
    if (this.hasNextPage) {
      this.loadEmails(this.currentPage + 1, this.searchQuery);
    }
  }

  prevPage() {
    if (this.hasPrevPage) {
      this.loadEmails(this.currentPage - 1, this.searchQuery);
    }
  }
}

// Factory function for creating new instances
export function createMailState() {
  return new MailState();
}
