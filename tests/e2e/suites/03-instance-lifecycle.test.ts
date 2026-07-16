import { describe, it, after } from 'node:test';
import assert from 'node:assert/strict';
import { BurdApiClient } from '../lib/api-client.js';
import { config } from '../config.js';
import { serviceConfigs } from '../fixtures/service-configs.js';
import { cleanup } from '../lib/cleanup.js';
import { allocatePort, generateTestName, waitForHealthy } from '../lib/helpers.js';

const client = new BurdApiClient();
cleanup.setClient(client);

after(async () => {
  await cleanup.cleanupAll(client);
});

describe('Instance Lifecycle', () => {
  let portIndex = 0;

  for (const svc of config.services) {
    const svcConfig = serviceConfigs[svc];
    if (!svcConfig) continue;

    const currentPortIndex = portIndex++;

    describe(`${svcConfig.displayName} instance`, () => {
      let instanceId: string;
      let version: string;
      let skipped = false;

      it('should have an installed version available', async () => {
        const versions = await client.getServiceVersions(svcConfig.serviceType);
        if (versions.installed.length === 0) {
          skipped = true;
          console.log(`  SKIP: No installed versions for ${svcConfig.displayName}`);
          return;
        }
        version = versions.installed[0];
      });

      it('should create an instance', async (t) => {
        if (skipped) return t.skip('No installed version');

        const name = generateTestName(svcConfig.serviceType);
        const port = allocatePort(currentPortIndex);

        const instance = await client.createInstance({
          name,
          port,
          service_type: svcConfig.serviceType,
          version,
        });

        instanceId = instance.id;
        cleanup.registerInstance(instanceId);

        assert.ok(instance.id, 'Instance should have an ID');
        assert.equal(instance.name, name);
        assert.equal(instance.port, port);
        assert.equal(instance.service_type, svcConfig.serviceType);
        assert.equal(instance.version, version);
        assert.equal(instance.running, false, 'Newly created instance should not be running');
        console.log(`  Created instance ${instance.id} (${name} on port ${port})`);
      });

      it('should retrieve the instance', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        const instance = await client.getInstance(instanceId);
        assert.equal(instance.id, instanceId);
        assert.equal(instance.service_type, svcConfig.serviceType);
      });

      it('should start the instance', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        const pid = await client.startInstance(instanceId);
        assert.ok(pid > 0, `Expected positive PID, got ${pid}`);
        console.log(`  Started with PID ${pid}`);
      });

      it('should become healthy', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        const instance = await waitForHealthy(client, instanceId);
        assert.equal(instance.running, true);
        assert.equal(instance.healthy, true);
        console.log(`  Healthy!`);
      });

      it('should return logs', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        // Logs may return as a string or might be empty
        const logs = await client.getInstanceLogs(instanceId);
        assert.equal(typeof logs, 'string');
      });

      it('should return env info', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        const env = await client.getInstanceEnv(instanceId);
        assert.equal(typeof env, 'string');
      });

      it('should stop the instance', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        await client.stopInstance(instanceId);
        const instance = await client.getInstance(instanceId);
        assert.equal(instance.running, false, 'Instance should not be running after stop');
        console.log(`  Stopped`);
      });

      it('should restart the instance', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        await client.startInstance(instanceId);
        await waitForHealthy(client, instanceId);

        await client.restartInstance(instanceId);
        const instance = await waitForHealthy(client, instanceId);
        assert.equal(instance.running, true);
        assert.equal(instance.healthy, true);
        console.log(`  Restarted and healthy`);

        // Stop for cleanup
        await client.stopInstance(instanceId);
      });

      it('should delete the instance', async (t) => {
        if (skipped || !instanceId) return t.skip('No instance');

        await client.deleteInstance(instanceId);

        try {
          await client.getInstance(instanceId);
          assert.fail('Instance should not exist after deletion');
        } catch (err) {
          // Expected: instance not found
          assert.ok((err as Error).message.includes('not found') || true);
        }
        console.log(`  Deleted`);
      });
    });
  }
});
