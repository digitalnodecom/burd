import { config } from '../config.js';
import type {
  ApiResponse,
  StatusResponse,
  InstanceResponse,
  DomainResponse,
  ServiceResponse,
  ServiceVersionsResponse,
  CreateInstanceRequest,
  CreateDomainRequest,
  UpdateDomainRequest,
  ToggleSslRequest,
} from './types.js';

export class BurdApiClient {
  private baseUrl: string;
  private timeout: number;

  constructor(baseUrl = config.apiBase, timeout = config.apiTimeout) {
    this.baseUrl = baseUrl;
    this.timeout = timeout;
  }

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    try {
      const res = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers: body ? { 'Content-Type': 'application/json' } : undefined,
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      const json = (await res.json()) as ApiResponse<T>;

      if (!json.success) {
        throw new Error(json.error || `API error: ${method} ${path} returned success=false`);
      }

      return json.data as T;
    } finally {
      clearTimeout(timer);
    }
  }

  private async requestVoid(method: string, path: string, body?: unknown): Promise<void> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeout);

    try {
      const res = await fetch(`${this.baseUrl}${path}`, {
        method,
        headers: body ? { 'Content-Type': 'application/json' } : undefined,
        body: body ? JSON.stringify(body) : undefined,
        signal: controller.signal,
      });

      const json = (await res.json()) as ApiResponse<unknown>;

      if (!json.success) {
        throw new Error(json.error || `API error: ${method} ${path} returned success=false`);
      }
    } finally {
      clearTimeout(timer);
    }
  }

  // Status
  async getStatus(): Promise<StatusResponse> {
    return this.request<StatusResponse>('GET', '/status');
  }

  // Services
  async listServices(): Promise<ServiceResponse[]> {
    return this.request<ServiceResponse[]>('GET', '/services');
  }

  async getServiceVersions(serviceType: string): Promise<ServiceVersionsResponse> {
    return this.request<ServiceVersionsResponse>('GET', `/services/${serviceType}/versions`);
  }

  // Instances
  async listInstances(): Promise<InstanceResponse[]> {
    return this.request<InstanceResponse[]>('GET', '/instances');
  }

  async createInstance(req: CreateInstanceRequest): Promise<InstanceResponse> {
    return this.request<InstanceResponse>('POST', '/instances', req);
  }

  async getInstance(id: string): Promise<InstanceResponse> {
    return this.request<InstanceResponse>('GET', `/instances/${id}`);
  }

  async startInstance(id: string): Promise<number> {
    const result = await this.request<{ pid: number }>('POST', `/instances/${id}/start`);
    return result.pid;
  }

  async stopInstance(id: string): Promise<void> {
    await this.requestVoid('POST', `/instances/${id}/stop`);
  }

  async restartInstance(id: string): Promise<void> {
    await this.requestVoid('POST', `/instances/${id}/restart`);
  }

  async deleteInstance(id: string): Promise<void> {
    await this.requestVoid('DELETE', `/instances/${id}`);
  }

  async getInstanceLogs(id: string): Promise<string> {
    return this.request<string>('GET', `/instances/${id}/logs`);
  }

  async getInstanceEnv(id: string): Promise<string> {
    return this.request<string>('GET', `/instances/${id}/env`);
  }

  // Domains
  async listDomains(): Promise<DomainResponse[]> {
    return this.request<DomainResponse[]>('GET', '/domains');
  }

  async createDomain(req: CreateDomainRequest): Promise<DomainResponse> {
    return this.request<DomainResponse>('POST', '/domains', req);
  }

  async updateDomain(id: string, req: UpdateDomainRequest): Promise<DomainResponse> {
    return this.request<DomainResponse>('PUT', `/domains/${id}`, req);
  }

  async deleteDomain(id: string): Promise<void> {
    await this.requestVoid('DELETE', `/domains/${id}`);
  }

  async toggleSsl(id: string, enabled: boolean): Promise<DomainResponse> {
    const body: ToggleSslRequest = { ssl_enabled: enabled };
    return this.request<DomainResponse>('POST', `/domains/${id}/ssl`, body);
  }
}
