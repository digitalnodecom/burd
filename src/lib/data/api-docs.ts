/**
 * API Documentation for The Burd Nest terminal
 * All endpoints available on localhost:19840
 */

export interface ApiEndpoint {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE';
  path: string;
  description: string;
  params?: string;
  body?: string;
  response?: string;
}

export interface ApiCategory {
  name: string;
  description: string;
  endpoints: ApiEndpoint[];
}

export const apiCategories: ApiCategory[] = [
  {
    name: 'Status',
    description: 'System status and health',
    endpoints: [
      {
        method: 'GET',
        path: '/status',
        description: 'Get overall Burd system status',
        response: `{
  "success": true,
  "data": {
    "app_running": true,
    "dns_running": true,
    "proxy_installed": true,
    "tld": "burd",
    "instance_count": 5,
    "running_instances": 3
  }
}`
      }
    ]
  },
  {
    name: 'Instances',
    description: 'Manage service instances (Redis, MariaDB, PostgreSQL, etc.)',
    endpoints: [
      {
        method: 'GET',
        path: '/instances',
        description: 'List all service instances with health status',
        response: `{
  "success": true,
  "data": [{
    "id": "uuid",
    "name": "my-redis",
    "port": 6379,
    "service_type": "Redis",
    "version": "7.2.4",
    "running": true,
    "healthy": true
  }]
}`
      },
      {
        method: 'GET',
        path: '/instances/:id',
        description: 'Get a specific instance by ID',
        params: ':id - Instance UUID'
      },
      {
        method: 'POST',
        path: '/instances',
        description: 'Create a new service instance',
        body: `{
  "name": "my-redis",
  "port": 6379,
  "service_type": "redis",
  "version": "7.2.4"
}`
      },
      {
        method: 'DELETE',
        path: '/instances/:id',
        description: 'Delete an instance (stops it first if running)',
        params: ':id - Instance UUID'
      },
      {
        method: 'POST',
        path: '/instances/:id/start',
        description: 'Start a stopped instance',
        params: ':id - Instance UUID'
      },
      {
        method: 'POST',
        path: '/instances/:id/stop',
        description: 'Stop a running instance',
        params: ':id - Instance UUID'
      },
      {
        method: 'POST',
        path: '/instances/:id/restart',
        description: 'Restart an instance',
        params: ':id - Instance UUID'
      },
      {
        method: 'GET',
        path: '/instances/:id/logs',
        description: 'Get recent logs from an instance',
        params: ':id - Instance UUID'
      },
      {
        method: 'GET',
        path: '/instances/:id/env',
        description: 'Get environment variables (DATABASE_URL, etc.)',
        params: ':id - Instance UUID',
        response: `{
  "success": true,
  "data": "REDIS_HOST=127.0.0.1\\nREDIS_PORT=6379..."
}`
      }
    ]
  },
  {
    name: 'Domains',
    description: 'Domain routing and SSL management',
    endpoints: [
      {
        method: 'GET',
        path: '/domains',
        description: 'List all configured domains',
        response: `{
  "success": true,
  "data": [{
    "id": "uuid",
    "subdomain": "api",
    "full_domain": "api.burd",
    "target_type": "instance",
    "target_value": "instance-uuid",
    "ssl_enabled": true
  }]
}`
      },
      {
        method: 'POST',
        path: '/domains',
        description: 'Create a new domain mapping',
        body: `{
  "subdomain": "api",
  "target_type": "instance|port|static",
  "target_value": "uuid|8080|/path/to/files",
  "ssl_enabled": false
}`
      },
      {
        method: 'PUT',
        path: '/domains/:id',
        description: 'Update a domain configuration',
        params: ':id - Domain UUID',
        body: `{
  "subdomain": "new-name",
  "target_type": "port",
  "target_value": "3000"
}`
      },
      {
        method: 'DELETE',
        path: '/domains/:id',
        description: 'Delete a domain',
        params: ':id - Domain UUID'
      },
      {
        method: 'POST',
        path: '/domains/:id/ssl',
        description: 'Enable or disable SSL for a domain',
        params: ':id - Domain UUID',
        body: `{ "ssl_enabled": true }`
      }
    ]
  },
  {
    name: 'Databases',
    description: 'Database management for MariaDB/PostgreSQL',
    endpoints: [
      {
        method: 'GET',
        path: '/databases',
        description: 'List all databases across all DB instances',
        response: `{
  "success": true,
  "data": [{
    "name": "myapp_dev",
    "instance_id": "uuid",
    "instance_name": "my-mariadb",
    "service_type": "MariaDB"
  }]
}`
      },
      {
        method: 'POST',
        path: '/databases',
        description: 'Create a new database',
        body: `{
  "name": "myapp_dev",
  "instance_id": "optional-specific-instance-uuid"
}`
      },
      {
        method: 'DELETE',
        path: '/databases/:name',
        description: 'Drop a database',
        params: ':name - Database name'
      }
    ]
  },
  {
    name: 'Services',
    description: 'Available service types and versions',
    endpoints: [
      {
        method: 'GET',
        path: '/services',
        description: 'List all available service types',
        response: `{
  "success": true,
  "data": [{
    "id": "redis",
    "name": "Redis",
    "default_port": 6379,
    "max_instances": null
  }]
}`
      },
      {
        method: 'GET',
        path: '/services/:type/versions',
        description: 'Get installed versions for a service',
        params: ':type - Service type (redis, mariadb, etc.)',
        response: `{
  "success": true,
  "data": {
    "service_type": "redis",
    "installed": ["7.2.4", "7.0.15"]
  }
}`
      }
    ]
  }
];

/**
 * Bird puns for the chirp command
 */
export const birdPuns: string[] = [
  "Why do birds fly south? It's too far to walk!",
  "This code is un-BURD-lievable!",
  "Nest practices for local development.",
  "404: Bird not found.",
  "Have you tried turning it off and nest again?",
  "It's not a bug, it's a feature... said the early bird.",
  "A bird in the terminal is worth two in the bush.",
  "Tweet your code with care!",
  "Owl always love good documentation.",
  "Don't count your databases before they're migrated.",
  "The early developer catches the bug.",
  "Feather in your cap for finding this easter egg!",
  "Robin your time with this hidden terminal.",
  "Let's talk about the bird and the bees... of microservices.",
  "Crow-ding conventions matter!",
  "Sparrow your thoughts on this API?",
  "Eagle-eyed developers find the best easter eggs.",
  "Toucan play at this game!",
  "Raven about your local dev setup!",
  "Penguin-tastic performance from localhost!",
];

/**
 * Get a random bird pun
 */
export function getRandomBirdPun(): string {
  return birdPuns[Math.floor(Math.random() * birdPuns.length)];
}

/**
 * Format an endpoint for display
 */
export function formatEndpoint(endpoint: ApiEndpoint): string {
  let output = `${endpoint.method} ${endpoint.path}\n`;
  output += `  ${endpoint.description}\n`;

  if (endpoint.params) {
    output += `  Params: ${endpoint.params}\n`;
  }

  if (endpoint.body) {
    output += `  Body:\n${endpoint.body.split('\n').map(l => '    ' + l).join('\n')}\n`;
  }

  if (endpoint.response) {
    output += `  Response:\n${endpoint.response.split('\n').map(l => '    ' + l).join('\n')}\n`;
  }

  return output;
}

/**
 * Get all endpoints as formatted text
 */
export function getAllEndpointsFormatted(): string {
  let output = '';
  for (const category of apiCategories) {
    output += `\n=== ${category.name.toUpperCase()} ===\n`;
    output += `${category.description}\n\n`;
    for (const endpoint of category.endpoints) {
      output += `  ${endpoint.method.padEnd(6)} ${endpoint.path}\n`;
      output += `         ${endpoint.description}\n\n`;
    }
  }
  return output;
}

/**
 * Get endpoints for a specific category
 */
export function getCategoryEndpoints(categoryName: string): string | null {
  const category = apiCategories.find(
    c => c.name.toLowerCase() === categoryName.toLowerCase()
  );

  if (!category) {
    return null;
  }

  let output = `\n=== ${category.name.toUpperCase()} ===\n`;
  output += `${category.description}\n\n`;

  for (const endpoint of category.endpoints) {
    output += formatEndpoint(endpoint) + '\n';
  }

  return output;
}
