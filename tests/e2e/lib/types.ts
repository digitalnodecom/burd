export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

export interface StatusResponse {
  app_running: boolean;
  dns_running: boolean;
  proxy_installed: boolean;
  tld: string;
  instance_count: number;
  running_instances: number;
}

export interface InstanceResponse {
  id: string;
  name: string;
  port: number;
  service_type: string;
  version: string;
  running: boolean;
  pid: number | null;
  healthy: boolean | null;
  domain: string | null;
  domain_enabled: boolean;
}

export interface DomainResponse {
  id: string;
  subdomain: string;
  full_domain: string;
  target_type: string;
  target_value: string;
  ssl_enabled: boolean;
}

export interface ServiceResponse {
  id: string;
  name: string;
  description: string;
  category: string;
}

export interface ServiceVersionsResponse {
  service_type: string;
  installed: string[];
}

export interface DatabaseResponse {
  name: string;
  instance_id: string;
  instance_name: string;
  service_type: string;
}

export interface CreateInstanceRequest {
  name: string;
  port: number;
  service_type: string;
  version: string;
  config?: Record<string, unknown>;
  custom_domain?: string;
}

export interface CreateDomainRequest {
  subdomain: string;
  target_type: 'instance' | 'port' | 'static';
  target_value: string;
  ssl_enabled?: boolean;
  static_browse?: boolean;
}

export interface UpdateDomainRequest {
  subdomain?: string;
  target_type?: string;
  target_value?: string;
}

export interface ToggleSslRequest {
  ssl_enabled: boolean;
}
