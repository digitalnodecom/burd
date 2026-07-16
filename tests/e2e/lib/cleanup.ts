import type { BurdApiClient } from './api-client.js';

class CleanupRegistry {
  private instanceIds: string[] = [];
  private domainIds: string[] = [];
  private client: BurdApiClient | null = null;

  setClient(client: BurdApiClient) {
    this.client = client;
  }

  registerInstance(id: string) {
    this.instanceIds.push(id);
  }

  registerDomain(id: string) {
    this.domainIds.push(id);
  }

  async cleanupAll(client?: BurdApiClient): Promise<void> {
    const c = client || this.client;
    if (!c) return;

    // Delete domains first (they may reference instances)
    for (const id of this.domainIds) {
      try {
        await c.deleteDomain(id);
      } catch (err) {
        console.error(`  cleanup: failed to delete domain ${id}:`, (err as Error).message);
      }
    }
    this.domainIds = [];

    // Stop and delete instances
    for (const id of this.instanceIds) {
      try {
        const instance = await c.getInstance(id);
        if (instance.running) {
          await c.stopInstance(id);
        }
      } catch {
        // Instance may already be gone
      }
      try {
        await c.deleteInstance(id);
      } catch (err) {
        console.error(`  cleanup: failed to delete instance ${id}:`, (err as Error).message);
      }
    }
    this.instanceIds = [];
  }
}

export const cleanup = new CleanupRegistry();

// Safety net: cleanup on unexpected exit
let cleanupScheduled = false;
function scheduleCleanup() {
  if (cleanupScheduled) return;
  cleanupScheduled = true;

  process.on('SIGINT', async () => {
    console.log('\nCleaning up test resources...');
    await cleanup.cleanupAll();
    process.exit(1);
  });
}

scheduleCleanup();
