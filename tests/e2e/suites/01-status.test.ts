import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { BurdApiClient } from '../lib/api-client.js';
import { config } from '../config.js';

const client = new BurdApiClient();

describe('Burd Status (prerequisites)', () => {
  it('should connect to the Burd API', async () => {
    try {
      await client.getStatus();
    } catch {
      assert.fail(
        `Cannot connect to Burd API at ${config.apiBase}. ` +
        'Make sure the Burd desktop app is running.'
      );
    }
  });

  it('should report app_running as true', async () => {
    const status = await client.getStatus();
    assert.equal(status.app_running, true, 'Burd app is not running');
  });

  it('should have DNS running', async () => {
    const status = await client.getStatus();
    assert.equal(status.dns_running, true, 'DNS server is not running');
  });

  it('should have a TLD configured', async () => {
    const status = await client.getStatus();
    assert.ok(status.tld, 'No TLD configured');
    console.log(`  TLD: ${status.tld}`);
  });

  it('should report proxy status', async () => {
    const status = await client.getStatus();
    console.log(`  Proxy installed: ${status.proxy_installed}`);
    console.log(`  Instances: ${status.instance_count} total, ${status.running_instances} running`);
  });
});
