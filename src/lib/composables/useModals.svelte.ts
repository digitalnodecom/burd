/**
 * Modal state management composable
 * Handles logs, env, settings, and instance settings modals
 */

import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, message } from "@tauri-apps/plugin-dialog";

// Types
interface InstanceConfigResponse {
  id: string;
  name: string;
  service_type: string;
  config: Record<string, unknown>;
}

interface ServiceInfo {
  id: string;
  display_name: string;
  default_port: number;
  config_fields: ConfigFieldInfo[];
  available: boolean;
  is_homebrew: boolean;
  process_manager: string;
}

interface ConfigFieldInfo {
  key: string;
  label: string;
  field_type: string;
  required: boolean;
  default: string | null;
}

interface Instance {
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
}

/**
 * Creates a logs modal state manager
 */
export function createLogsModal() {
  let show = $state(false);
  let content = $state("");
  let instanceName = $state("");
  let instanceId = $state("");
  let loading = $state(false);

  async function open(id: string, name: string) {
    try {
      loading = true;
      instanceId = id;
      instanceName = name;
      show = true;
      content = await invoke<string>("get_instance_logs", { id });
    } catch (e) {
      content = `Error loading logs: ${e}`;
    } finally {
      loading = false;
    }
  }

  async function refresh() {
    if (!instanceId) return;
    try {
      loading = true;
      content = await invoke<string>("get_instance_logs", { id: instanceId });
    } catch (e) {
      content = `Error loading logs: ${e}`;
    } finally {
      loading = false;
    }
  }

  function close() {
    show = false;
    content = "";
    instanceName = "";
    instanceId = "";
  }

  return {
    get show() { return show; },
    set show(v: boolean) { show = v; },
    get content() { return content; },
    get instanceName() { return instanceName; },
    get instanceId() { return instanceId; },
    get loading() { return loading; },
    open,
    refresh,
    close,
  };
}

/**
 * Creates an env modal state manager
 */
export function createEnvModal() {
  let show = $state(false);
  let content = $state("");
  let instanceName = $state("");
  let serviceType = $state("");
  let loading = $state(false);
  let copied = $state(false);

  async function open(id: string, name: string, type: string) {
    try {
      loading = true;
      instanceName = name;
      serviceType = type;
      copied = false;
      show = true;
      content = await invoke<string>("get_instance_env", { id });
    } catch (e) {
      content = `Error loading ENV: ${e}`;
    } finally {
      loading = false;
    }
  }

  async function copyToClipboard(): Promise<string | null> {
    try {
      await navigator.clipboard.writeText(content);
      copied = true;
      setTimeout(() => { copied = false; }, 2000);
      return null;
    } catch (e) {
      return `Failed to copy: ${e}`;
    }
  }

  function close() {
    show = false;
    content = "";
    instanceName = "";
    serviceType = "";
    copied = false;
  }

  return {
    get show() { return show; },
    set show(v: boolean) { show = v; },
    get content() { return content; },
    get instanceName() { return instanceName; },
    get serviceType() { return serviceType; },
    get loading() { return loading; },
    get copied() { return copied; },
    open,
    copyToClipboard,
    close,
  };
}

/**
 * Creates a settings modal state manager
 */
export function createSettingsModal() {
  let show = $state(false);
  let tld = $state("");
  let saving = $state(false);

  function open(currentTld: string) {
    tld = currentTld;
    show = true;
  }

  async function save(): Promise<string | null> {
    if (!tld.trim()) {
      return "TLD cannot be empty";
    }
    try {
      saving = true;
      await invoke("update_tld", { tld: tld.trim() });
      show = false;
      await message("TLD updated! Please restart the app for changes to take effect.", {
        title: "TLD Updated",
        kind: "info"
      });
      return null;
    } catch (e) {
      return String(e);
    } finally {
      saving = false;
    }
  }

  function close() {
    show = false;
  }

  return {
    get show() { return show; },
    set show(v: boolean) { show = v; },
    get tld() { return tld; },
    set tld(v: string) { tld = v; },
    get saving() { return saving; },
    open,
    save,
    close,
  };
}

/**
 * Creates an instance settings modal state manager
 */
export function createInstanceSettingsModal(
  getServiceTypes: () => ServiceInfo[],
  getInstances: () => Instance[],
  onRefresh: () => Promise<void>
) {
  let show = $state(false);
  let instanceId = $state("");
  let instanceName = $state("");
  let serviceType = $state("");
  let config = $state<Record<string, string>>({});
  let loading = $state(false);
  let saving = $state(false);

  async function open(instance: Instance): Promise<string | null> {
    try {
      loading = true;
      instanceId = instance.id;
      instanceName = instance.name;
      show = true;
      const result = await invoke<InstanceConfigResponse>("get_instance_config", { id: instance.id });
      serviceType = result.service_type;
      const newConfig: Record<string, string> = {};
      const serviceMeta = getServiceTypes().find(s => s.id === result.service_type);
      if (serviceMeta?.config_fields) {
        for (const field of serviceMeta.config_fields) {
          const value = result.config[field.key];
          newConfig[field.key] = value !== undefined && value !== null ? String(value) : (field.default || "");
        }
      }
      config = newConfig;
      return null;
    } catch (e) {
      show = false;
      return String(e);
    } finally {
      loading = false;
    }
  }

  async function save(): Promise<string | null> {
    try {
      saving = true;
      await invoke("update_instance_config", { id: instanceId, config });
      show = false;
      const instance = getInstances().find(i => i.id === instanceId);
      if (instance?.running) {
        await message("Settings saved! Restart the instance for changes to take effect.", {
          title: "Settings Saved",
          kind: "info"
        });
      }
      await onRefresh();
      return null;
    } catch (e) {
      return String(e);
    } finally {
      saving = false;
    }
  }

  async function browseFolder(fieldKey: string): Promise<string | null> {
    try {
      const selected = await openDialog({ directory: true, multiple: false, title: "Select Folder" });
      if (selected && typeof selected === "string") {
        config = { ...config, [fieldKey]: selected };
      }
      return null;
    } catch (e) {
      return String(e);
    }
  }

  function getServiceMeta(): ServiceInfo | undefined {
    return getServiceTypes().find(s => s.id === serviceType);
  }

  function updateConfig(key: string, value: string) {
    config = { ...config, [key]: value };
  }

  function close() {
    show = false;
  }

  return {
    get show() { return show; },
    set show(v: boolean) { show = v; },
    get instanceId() { return instanceId; },
    get instanceName() { return instanceName; },
    get serviceType() { return serviceType; },
    get config() { return config; },
    get loading() { return loading; },
    get saving() { return saving; },
    open,
    save,
    browseFolder,
    getServiceMeta,
    updateConfig,
    close,
  };
}
