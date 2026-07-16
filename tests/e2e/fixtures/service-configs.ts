export interface ServiceTestConfig {
  serviceType: string;
  displayName: string;
  healthCheckType: 'http' | 'tcp';
  healthPath?: string;
  startupTime: number;
}

export const serviceConfigs: Record<string, ServiceTestConfig> = {
  redis: {
    serviceType: 'redis',
    displayName: 'Redis',
    healthCheckType: 'tcp',
    startupTime: 3_000,
  },
  meilisearch: {
    serviceType: 'meilisearch',
    displayName: 'Meilisearch',
    healthCheckType: 'http',
    healthPath: '/health',
    startupTime: 5_000,
  },
  mailpit: {
    serviceType: 'mailpit',
    displayName: 'Mailpit',
    healthCheckType: 'http',
    healthPath: '/livez',
    startupTime: 3_000,
  },
  valkey: {
    serviceType: 'valkey',
    displayName: 'Valkey',
    healthCheckType: 'tcp',
    startupTime: 3_000,
  },
  memcached: {
    serviceType: 'memcached',
    displayName: 'Memcached',
    healthCheckType: 'tcp',
    startupTime: 3_000,
  },
  beanstalkd: {
    serviceType: 'beanstalkd',
    displayName: 'Beanstalkd',
    healthCheckType: 'tcp',
    startupTime: 3_000,
  },
};
