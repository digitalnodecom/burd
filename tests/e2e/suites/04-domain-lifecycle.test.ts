import { describe, it, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { BurdApiClient } from '../lib/api-client.js';
import { config } from '../config.js';
import { serviceConfigs } from '../fixtures/service-configs.js';
import { cleanup } from '../lib/cleanup.js';
import { allocatePort, generateTestName, generateTestSubdomain, waitForHealthy } from '../lib/helpers.js';

const client = new BurdApiClient();
cleanup.setClient(client);

after(async () => {
  await cleanup.cleanupAll(client);
});

describe('Domain Lifecycle', () => {
  let instanceId: string;
  let instancePort: number;
  let tld: string;
  let skipped = false;

  before(async () => {
    // Get TLD
    const status = await client.getStatus();
    tld = status.tld;

    // Find a service with an installed version to create a backing instance
    for (const svc of config.services) {
      const svcConfig = serviceConfigs[svc];
      if (!svcConfig) continue;

      const versions = await client.getServiceVersions(svcConfig.serviceType);
      if (versions.installed.length === 0) continue;

      instancePort = allocatePort(50); // Use a high index to avoid conflicts with instance lifecycle tests
      const instance = await client.createInstance({
        name: generateTestName('domain-backing'),
        port: instancePort,
        service_type: svcConfig.serviceType,
        version: versions.installed[0],
      });

      instanceId = instance.id;
      cleanup.registerInstance(instanceId);

      await client.startInstance(instanceId);
      await waitForHealthy(client, instanceId);
      console.log(`  Backing instance: ${instance.id} (${svcConfig.displayName} on port ${instancePort})`);
      return;
    }

    skipped = true;
    console.log('  SKIP: No services with installed versions available for domain tests');
  });

  it('should create a domain targeting an instance', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const subdomain = generateTestSubdomain('inst');
    const domain = await client.createDomain({
      subdomain,
      target_type: 'instance',
      target_value: instanceId,
    });

    cleanup.registerDomain(domain.id);

    assert.equal(domain.subdomain, subdomain);
    assert.equal(domain.full_domain, `${subdomain}.${tld}`);
    assert.equal(domain.target_type, 'instance');
    assert.equal(domain.target_value, instanceId);
    assert.equal(domain.ssl_enabled, false);
    console.log(`  Created domain: ${domain.full_domain} -> instance ${instanceId}`);
  });

  it('should create a domain targeting a port', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const subdomain = generateTestSubdomain('port');
    const domain = await client.createDomain({
      subdomain,
      target_type: 'port',
      target_value: String(instancePort),
    });

    cleanup.registerDomain(domain.id);

    assert.equal(domain.subdomain, subdomain);
    assert.equal(domain.target_type, 'port');
    assert.equal(domain.target_value, String(instancePort));
    console.log(`  Created domain: ${domain.full_domain} -> port ${instancePort}`);
  });

  it('should list domains including test domains', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const domains = await client.listDomains();
    const testDomains = domains.filter(d => d.subdomain.startsWith('e2e-'));
    assert.ok(testDomains.length >= 2, `Expected at least 2 test domains, got ${testDomains.length}`);
  });

  it('should toggle SSL on', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const domains = await client.listDomains();
    const testDomain = domains.find(d => d.subdomain.startsWith('e2e-inst'));
    assert.ok(testDomain, 'Test instance domain not found');

    const updated = await client.toggleSsl(testDomain.id, true);
    assert.equal(updated.ssl_enabled, true);
    console.log(`  SSL enabled on ${updated.full_domain}`);
  });

  it('should toggle SSL off', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const domains = await client.listDomains();
    const testDomain = domains.find(d => d.subdomain.startsWith('e2e-inst'));
    assert.ok(testDomain, 'Test instance domain not found');

    const updated = await client.toggleSsl(testDomain.id, false);
    assert.equal(updated.ssl_enabled, false);
    console.log(`  SSL disabled on ${updated.full_domain}`);
  });

  it('should update a domain subdomain', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const domains = await client.listDomains();
    const testDomain = domains.find(d => d.subdomain.startsWith('e2e-port'));
    assert.ok(testDomain, 'Test port domain not found');

    const newSubdomain = generateTestSubdomain('renamed');
    const updated = await client.updateDomain(testDomain.id, { subdomain: newSubdomain });
    assert.equal(updated.subdomain, newSubdomain);
    console.log(`  Renamed domain to ${updated.full_domain}`);
  });

  it('should reject duplicate subdomains', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const domains = await client.listDomains();
    const testDomain = domains.find(d => d.subdomain.startsWith('e2e-'));
    assert.ok(testDomain, 'No test domain found');

    try {
      await client.createDomain({
        subdomain: testDomain.subdomain,
        target_type: 'port',
        target_value: '9999',
      });
      assert.fail('Should have rejected duplicate subdomain');
    } catch (err) {
      // Expected error
      assert.ok((err as Error).message.length > 0);
      console.log(`  Correctly rejected duplicate: ${(err as Error).message}`);
    }
  });

  it('should delete test domains', async (t) => {
    if (skipped) return t.skip('No backing instance');

    const domains = await client.listDomains();
    const testDomains = domains.filter(d => d.subdomain.startsWith('e2e-'));

    for (const domain of testDomains) {
      await client.deleteDomain(domain.id);
      console.log(`  Deleted domain: ${domain.full_domain}`);
    }

    const remaining = await client.listDomains();
    const leftover = remaining.filter(d => d.subdomain.startsWith('e2e-'));
    assert.equal(leftover.length, 0, 'All test domains should be deleted');
  });
});
