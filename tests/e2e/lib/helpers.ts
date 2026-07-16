import { config } from '../config.js';
import type { BurdApiClient } from './api-client.js';
import type { InstanceResponse } from './types.js';

export function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms));
}

export function allocatePort(index: number): number {
  return config.portRangeStart + index * 10;
}

export function generateTestName(serviceType: string): string {
  return `${config.testPrefix}-${serviceType}-${Date.now()}`;
}

export function generateTestSubdomain(serviceType: string): string {
  return `e2e-${serviceType}-${Date.now()}`;
}

export async function waitForHealthy(
  client: BurdApiClient,
  instanceId: string,
  timeout = config.healthCheckTimeout,
): Promise<InstanceResponse> {
  const deadline = Date.now() + timeout;

  // Initial grace period before polling
  await sleep(config.startupGracePeriod);

  while (Date.now() < deadline) {
    const instance = await client.getInstance(instanceId);
    if (instance.healthy === true) {
      return instance;
    }
    await sleep(config.healthCheckInterval);
  }

  // One final check
  const instance = await client.getInstance(instanceId);
  if (instance.healthy === true) {
    return instance;
  }

  throw new Error(
    `Instance ${instanceId} did not become healthy within ${timeout}ms ` +
    `(running=${instance.running}, healthy=${instance.healthy})`
  );
}
