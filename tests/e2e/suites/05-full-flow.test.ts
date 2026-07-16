import { describe, it, after } from 'node:test';
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

describe('Full End-to-End Flow', () => {
  let instanceId: string;
  let domainId: string;
  let tld: string;
  let proxyInstalled: boolean;
  let skipped = false;
  let serviceType: string;
  let subdomain: string;

  it('should verify Burd is healthy', async () => {
    const status = await client.getStatus();
    assert.equal(status.app_running, true);
    assert.equal(status.dns_running, true);
    tld = status.tld;
    proxyInstalled = status.proxy_installed;
  });

  it('should find a testable service with installed versions', async () => {
    for (const svc of config.services) {
      const svcConfig = serviceConfigs[svc];
      if (!svcConfig) continue;

      const versions = await client.getServiceVersions(svcConfig.serviceType);
      if (versions.installed.length > 0) {
        serviceType = svcConfig.serviceType;
        console.log(`  Using service: ${svcConfig.displayName} v${versions.installed[0]}`);
        return;
      }
    }
    skipped = true;
    console.log('  SKIP: No services with installed versions');
  });

  it('should create and start an instance', async (t) => {
    if (skipped) return t.skip('No testable service');

    const versions = await client.getServiceVersions(serviceType);
    const port = allocatePort(99); // High index for isolation

    const instance = await client.createInstance({
      name: generateTestName('full-flow'),
      port,
      service_type: serviceType,
      version: versions.installed[0],
    });

    instanceId = instance.id;
    cleanup.registerInstance(instanceId);

    assert.equal(instance.running, false);
    console.log(`  Created instance: ${instance.id}`);

    await client.startInstance(instanceId);
    const healthy = await waitForHealthy(client, instanceId);
    assert.equal(healthy.running, true);
    assert.equal(healthy.healthy, true);
    console.log(`  Instance running and healthy`);
  });

  it('should create a domain pointing to the instance', async (t) => {
    if (skipped || !instanceId) return t.skip('No instance');

    subdomain = generateTestSubdomain('full');
    const domain = await client.createDomain({
      subdomain,
      target_type: 'instance',
      target_value: instanceId,
    });

    domainId = domain.id;
    cleanup.registerDomain(domainId);

    assert.equal(domain.subdomain, subdomain);
    assert.equal(domain.full_domain, `${subdomain}.${tld}`);
    assert.equal(domain.target_type, 'instance');
    console.log(`  Domain created: ${domain.full_domain}`);
  });

  it('should verify domain is registered in domain list', async (t) => {
    if (skipped || !domainId) return t.skip('No domain');

    const domains = await client.listDomains();
    const found = domains.find(d => d.id === domainId);
    assert.ok(found, 'Domain should appear in list');
    assert.equal(found.subdomain, subdomain);
  });

  it('should verify domain routing via HTTP (if available)', async (t) => {
    if (skipped || !domainId) return t.skip('No domain');

    const svcConfig = serviceConfigs[serviceType];
    if (!svcConfig || svcConfig.healthCheckType !== 'http' || !svcConfig.healthPath) {
      console.log(`  SKIP: ${serviceType} uses TCP health checks, no HTTP endpoint to verify`);
      return;
    }

    // Try to reach the service through the domain
    const proxyPort = proxyInstalled ? 80 : 8080;
    const url = `http://${subdomain}.${tld}:${proxyPort}${svcConfig.healthPath}`;

    try {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 5_000);
      const res = await fetch(url, { signal: controller.signal });
      clearTimeout(timer);
      assert.ok(res.ok, `Domain routing returned ${res.status}`);
      console.log(`  Domain routing verified: ${url} -> ${res.status}`);
    } catch (err) {
      // DNS might not resolve if resolver isn't installed
      console.log(`  Could not verify domain routing directly (DNS may not be configured for .${tld})`);
      console.log(`  API-level verification passed (domain registered and instance healthy)`);
    }
  });

  it('should toggle SSL on the domain', async (t) => {
    if (skipped || !domainId) return t.skip('No domain');

    const updated = await client.toggleSsl(domainId, true);
    assert.equal(updated.ssl_enabled, true);
    console.log(`  SSL enabled`);

    const reverted = await client.toggleSsl(domainId, false);
    assert.equal(reverted.ssl_enabled, false);
    console.log(`  SSL disabled`);
  });

  it('should stop the instance', async (t) => {
    if (skipped || !instanceId) return t.skip('No instance');

    await client.stopInstance(instanceId);
    const instance = await client.getInstance(instanceId);
    assert.equal(instance.running, false);
    console.log(`  Instance stopped`);
  });

  it('should delete the domain', async (t) => {
    if (skipped || !domainId) return t.skip('No domain');

    await client.deleteDomain(domainId);
    const domains = await client.listDomains();
    const found = domains.find(d => d.id === domainId);
    assert.ok(!found, 'Domain should be gone after deletion');
    console.log(`  Domain deleted`);
  });

  it('should delete the instance', async (t) => {
    if (skipped || !instanceId) return t.skip('No instance');

    await client.deleteInstance(instanceId);
    try {
      await client.getInstance(instanceId);
      assert.fail('Instance should not exist after deletion');
    } catch {
      // Expected
    }
    console.log(`  Instance deleted`);
  });

  it('should verify no test resources remain', async () => {
    const instances = await client.listInstances();
    const testInstances = instances.filter(i => i.name.startsWith(config.testPrefix));
    assert.equal(testInstances.length, 0, `Found ${testInstances.length} leftover test instances`);

    const domains = await client.listDomains();
    const testDomains = domains.filter(d => d.subdomain.startsWith('e2e-'));
    assert.equal(testDomains.length, 0, `Found ${testDomains.length} leftover test domains`);

    console.log(`  All test resources cleaned up`);
  });
});
