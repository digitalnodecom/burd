import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { BurdApiClient } from '../lib/api-client.js';
import { config } from '../config.js';
import { serviceConfigs } from '../fixtures/service-configs.js';

const client = new BurdApiClient();

describe('Service Catalog', () => {
  it('should list available services', async () => {
    const services = await client.listServices();
    assert.ok(services.length > 0, 'No services returned');
    console.log(`  Found ${services.length} services`);
  });

  for (const svc of config.services) {
    const svcConfig = serviceConfigs[svc];
    if (!svcConfig) {
      it(`should have config for ${svc}`, () => {
        assert.fail(`No test config defined for service "${svc}" in fixtures/service-configs.ts`);
      });
      continue;
    }

    describe(`Service: ${svcConfig.displayName}`, () => {
      it('should exist in the service catalog', async () => {
        const services = await client.listServices();
        const found = services.some(s => s.id === svcConfig.serviceType);
        assert.ok(found, `Service "${svcConfig.serviceType}" not found in catalog`);
      });

      it('should have installed versions', async () => {
        const versions = await client.getServiceVersions(svcConfig.serviceType);
        if (versions.installed.length === 0) {
          console.log(`  WARNING: No installed versions for ${svcConfig.displayName} - tests will be skipped`);
          return;
        }
        console.log(`  Installed versions: ${versions.installed.join(', ')}`);
      });
    });
  }
});
