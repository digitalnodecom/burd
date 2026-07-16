export const config = {
  apiBase: process.env.BURD_API_URL || 'http://127.0.0.1:19840',

  // Timeouts (ms)
  healthCheckTimeout: 30_000,
  healthCheckInterval: 1_000,
  apiTimeout: 10_000,
  startupGracePeriod: 2_000,

  // Port range for test instances (well above typical dev ports)
  portRangeStart: 29_000,

  // Which services to test (override via BURD_TEST_SERVICES env var)
  services: (process.env.BURD_TEST_SERVICES || 'redis,meilisearch,mailpit').split(',').map(s => s.trim()),

  // Test instance/domain name prefix for identification and cleanup
  testPrefix: 'e2e-test',
};
