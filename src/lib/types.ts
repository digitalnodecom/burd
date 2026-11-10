export interface Instance {
  id: string;
  name: string;
  port: number;
  service_type: string;
  version: string;
  running: boolean;
  pid: number | null;
  healthy: boolean | null;
  has_config: boolean;
  domain: string;
  domain_enabled: boolean;
  process_manager: string;
  stack_id: string | null;
  mapped_domains: string[];
  auto_start: boolean;
}

export interface ServiceInfo {
  id: string;
  display_name: string;
  default_port: number;
  config_fields: ConfigFieldInfo[];
  available: boolean;
  is_homebrew: boolean;
  process_manager: string;
}

export interface ConfigFieldInfo {
  key: string;
  label: string;
  field_type: string;
  required: boolean;
  default: string | null;
}

export interface InstanceConfigResponse {
    id: string;
    name: string;
    service_type: string;
    config: Record<string, unknown>;
  }

export interface InstanceInfo {
    id: string;
    name: string;
    service_type: string;
    version: string;
    port: number;
    running: boolean;
    pid: number | null;
    categories: InfoCategory[];
  }

export interface InfoCategory {
    title: string;
    items: InfoItem[];
  }

export interface InfoItem {
    label: string;
    value: string;
    copyable: boolean;
  }

export interface InstanceRowActions {
  onSelect: (id: string) => void;
  onDragStart: (event: MouseEvent, id: string) => void;
  onReorder: (instance: Instance, direction: number) => void;
  onCopy: (value: string, id: string) => void;
  onDeleteDomain: (domain: string, id: string) => void;
  onStart: (id: string) => void;
  onStop: (id: string) => void;
  onSettings: (instance: Instance) => void;
  onLogs: (id: string, name: string) => void;
  onEnv: (id: string, name: string, serviceType: string) => void;
  onInfo: (id: string, name: string, serviceType: string) => void;
  onDelete: (id: string, name: string) => void;
  onDatabases: (instance: Instance) => void;
  onExtensions: (instance: Instance) => void;
}
