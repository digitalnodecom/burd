<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open, confirm, message } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { useKonamiCode } from "$lib/composables/useKonamiCode.svelte";

  // Import components
  import Sidebar from "$lib/components/Sidebar.svelte";
  import BurdNest from "$lib/components/BurdNest.svelte";
  import ImportStackModal from "$lib/components/ImportStackModal.svelte";
  import ExportStackModal from "$lib/components/ExportStackModal.svelte";
  import DeleteStackModal from "$lib/components/DeleteStackModal.svelte";
  import GeneralSection from "$lib/sections/GeneralSection.svelte";
  import DomainsSection from "$lib/sections/DomainsSection.svelte";
  import TunnelsSection from "$lib/sections/TunnelsSection.svelte";
  import InstancesSection from "$lib/sections/InstancesSection.svelte";
  import ServicesSection from "$lib/sections/ServicesSection.svelte";
  import NodeSection from "$lib/sections/NodeSection.svelte";
  import PHPSection from "$lib/sections/PHPSection.svelte";
  import LogsSection from "$lib/sections/LogsSection.svelte";
  import ParksSection from "$lib/sections/ParksSection.svelte";
  import TinkerSection from "$lib/sections/TinkerSection.svelte";
  import MailSection from "$lib/sections/MailSection.svelte";

  // === Type Definitions ===
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

  interface Stack {
    id: string;
    name: string;
    description: string | null;
    created_at: string;
    updated_at: string;
  }

  interface NetworkStatus {
    dns_running: boolean;
    dns_port: number;
    proxy_running: boolean;
    proxy_port: number;
    resolver_installed: boolean;
    active_routes: RouteInfo[];
    tld: string;
  }

  interface AppSettings {
    tld: string;
    dns_port: number;
    proxy_port: number;
  }

  interface RouteInfo {
    domain: string;
    port: number;
    instance_id: string;
  }

  interface ProxyStatus {
    daemon_installed: boolean;
    daemon_running: boolean;
    daemon_pid: number | null;
    caddy_installed: boolean;
  }

  interface CliStatus {
    installed: boolean;
    path: string | null;
    binary_exists: boolean;
  }

  interface HelperStatus {
    installed: boolean;
    running: boolean;
  }

  interface CATrustStatus {
    ca_exists: boolean;
    is_trusted: boolean;
    ca_path: string;
    cert_name: string | null;
    cert_expiry: string | null;
  }

  interface BinaryStatus {
    service_type: string;
    installed: boolean;
    version: string | null;
    path: string | null;
  }

  interface DownloadProgress {
    service_type: string;
    downloaded: number;
    total: number;
    percentage: number;
    phase: string; // "downloading", "extracting", "installing", "complete"
  }

  interface VersionInfo {
    version: string;
    is_latest: boolean;
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

  interface InstanceConfigResponse {
    id: string;
    name: string;
    service_type: string;
    config: Record<string, unknown>;
  }

  interface InstanceInfo {
    id: string;
    name: string;
    service_type: string;
    version: string;
    port: number;
    running: boolean;
    pid: number | null;
    categories: InfoCategory[];
  }

  interface InfoCategory {
    title: string;
    items: InfoItem[];
  }

  interface InfoItem {
    label: string;
    value: string;
    copyable: boolean;
  }

  // === Navigation State ===
  let activeSection = $state("general");

  // === Data State ===
  let serviceTypes = $state<ServiceInfo[]>([]);
  let instances = $state<Instance[]>([]);
  let stacks = $state<Stack[]>([]);
  let binaryStatuses = $state<BinaryStatus[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let downloading = $state<Record<string, boolean>>({});
  let downloadProgress = $state<Record<string, DownloadProgress>>({});

  // Version selection for downloads
  let availableVersions = $state<Record<string, VersionInfo[]>>({});
  let selectedVersions = $state<Record<string, string>>({});
  let loadingVersions = $state<Record<string, boolean>>({});
  let showVersionSelector = $state<Record<string, boolean>>({});

  // Installed versions per service
  let installedVersions = $state<Record<string, string[]>>({});

  // Action states
  let actionLoading = $state<Record<string, boolean>>({});

  // Node-RED initialization states
  let initializingInstances = $state<Record<string, boolean>>({});
  let initializedInstances = $state<Record<string, boolean>>({});

  // Logs modal
  let showLogs = $state(false);
  let logsContent = $state("");
  let logsInstanceName = $state("");
  let logsLoading = $state(false);

  // ENV modal
  let showEnv = $state(false);
  let envContent = $state("");
  let envInstanceName = $state("");
  let envServiceType = $state("");
  let envLoading = $state(false);
  let envCopied = $state(false);

  // Info modal
  let showInfo = $state(false);
  let infoContent = $state<InstanceInfo | null>(null);
  let infoInstanceName = $state("");
  let infoServiceType = $state("");
  let infoLoading = $state(false);
  let infoError = $state<string | null>(null);

  // Network status
  let networkStatus = $state<NetworkStatus | null>(null);
  let installingResolver = $state(false);
  let dnsServerAction = $state(false);

  // Reverse Proxy status
  let proxyStatus = $state<ProxyStatus | null>(null);
  let settingUpProxy = $state(false);
  let disablingProxy = $state(false);
  let startingDaemon = $state(false);
  let restartingDaemon = $state(false);

  // CLI status
  let cliStatus = $state<CliStatus | null>(null);
  let installingCli = $state(false);

  // Helper status
  let helperStatus = $state<HelperStatus | null>(null);
  let installingHelper = $state(false);

  // CA Trust status
  let caTrustStatus = $state<CATrustStatus | null>(null);
  let trustingCA = $state(false);

  // Park status
  let parkEnabled = $state(false);

  // Mail status (derived from instances)
  let mailpitExists = $derived(instances.some(i => i.service_type.toLowerCase() === 'mailpit'));
  let unreadMailCount = $state(0);

  // Tunnel status (derived from instances)
  let frpcInstanceExists = $derived(instances.some(i => i.service_type.toLowerCase() === 'frpc'));

  // Burd Nest (easter egg terminal)
  let showBurdNest = $state(false);

  // Import Stack Modal
  let showImportStackModal = $state(false);

  // Export Stack Modal
  let showExportStackModal = $state(false);
  let exportingStack = $state<Stack | null>(null);

  // Delete Stack Modal
  let showDeleteStackModal = $state(false);
  let deletingStack = $state<Stack | null>(null);

  // Settings Modal
  let showSettings = $state(false);
  let settingsTld = $state("");
  let savingSettings = $state(false);

  // Instance Settings Modal
  let showInstanceSettings = $state(false);
  let instanceSettingsId = $state("");
  let instanceSettingsName = $state("");
  let instanceSettingsOriginalName = $state("");
  let instanceSettingsServiceType = $state("");
  let instanceSettingsVersion = $state("");
  let instanceSettingsOriginalVersion = $state("");
  let instanceSettingsInstalledVersions = $state<string[]>([]);
  let instanceSettingsConfig = $state<Record<string, string>>({});
  let instanceSettingsLoading = $state(false);
  let instanceSettingsSaving = $state(false);

  // Instance Settings - Domain Management
  let instanceSettingsDomains = $state<string[]>([]);
  let instanceSettingsAllDomains = $state<any[]>([]);
  let showDomainForm = $state(false);
  let newDomainSubdomain = $state("");
  let newDomainSsl = $state(true);

  // === Data Loading ===
  async function loadData() {
    try {
      loading = true;
      error = null;
      const [instancesResult, stacksResult, binaryResult, servicesResult, networkResult, proxyResult, cliResult, helperResult, parkEnabledResult, caTrustResult] = await Promise.all([
        invoke<Instance[]>("list_instances"),
        invoke<Stack[]>("list_stacks"),
        invoke<BinaryStatus[]>("get_all_binary_statuses"),
        invoke<ServiceInfo[]>("get_available_services"),
        invoke<NetworkStatus>("get_network_status"),
        invoke<ProxyStatus>("get_proxy_status"),
        invoke<CliStatus>("get_cli_status"),
        invoke<HelperStatus>("get_helper_status"),
        invoke<boolean>("is_park_enabled"),
        invoke<CATrustStatus>("get_ca_trust_status"),
      ]);
      instances = instancesResult;
      stacks = stacksResult;
      binaryStatuses = binaryResult;
      networkStatus = networkResult;
      proxyStatus = proxyResult;
      cliStatus = cliResult;
      helperStatus = helperResult;
      parkEnabled = parkEnabledResult;
      caTrustStatus = caTrustResult;
      if (servicesResult.length > 0) {
        serviceTypes = servicesResult.filter(s => s.available);
      }
      // Fetch installed versions for all services
      for (const svc of serviceTypes) {
        try {
          const versions = await invoke<string[]>("get_installed_versions", { serviceType: svc.id });
          installedVersions = { ...installedVersions, [svc.id]: versions };
        } catch {
          installedVersions = { ...installedVersions, [svc.id]: [] };
        }
      }
      // Check initialization status for Node-RED instances
      const noderedInstances = instancesResult.filter(i => i.service_type.toLowerCase() === 'nodered');
      for (const instance of noderedInstances) {
        try {
          const isInit = await invoke<boolean>("is_nodered_initialized", { id: instance.id });
          initializedInstances = { ...initializedInstances, [instance.id]: isInit };
        } catch {
          initializedInstances = { ...initializedInstances, [instance.id]: false };
        }
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // === Service/Binary Functions ===
  async function fetchVersions(serviceType: string) {
    if (availableVersions[serviceType]?.length > 0) {
      showVersionSelector = { ...showVersionSelector, [serviceType]: true };
      return;
    }
    try {
      loadingVersions = { ...loadingVersions, [serviceType]: true };
      const versions = await invoke<VersionInfo[]>("get_available_versions", { serviceType });
      availableVersions = { ...availableVersions, [serviceType]: versions };
      if (versions.length > 0) {
        selectedVersions = { ...selectedVersions, [serviceType]: versions[0].version };
      }
      showVersionSelector = { ...showVersionSelector, [serviceType]: true };
    } catch (e) {
      error = String(e);
    } finally {
      loadingVersions = { ...loadingVersions, [serviceType]: false };
    }
  }

  async function downloadBinary(serviceType: string, version: string) {
    if (!version) {
      error = "Please select a version first";
      return;
    }
    try {
      downloading = { ...downloading, [serviceType]: true };
      showVersionSelector = { ...showVersionSelector, [serviceType]: false };
      downloadProgress = { ...downloadProgress, [serviceType]: { service_type: serviceType, downloaded: 0, total: 0, percentage: 0, phase: "downloading" }};
      await invoke("download_binary", { serviceType, version });
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      downloading = { ...downloading, [serviceType]: false };
      // Remove this service's progress
      const { [serviceType]: _, ...rest } = downloadProgress;
      downloadProgress = rest;
    }
  }

  async function deleteVersion(serviceType: string, version: string) {
    const confirmed = await confirm(
      `Delete ${serviceType} version ${version}?`,
      { title: "Delete Version", kind: "warning" }
    );
    if (!confirmed) return;
    try {
      await invoke("delete_binary_version", { serviceType, version });
      await loadData();
    } catch (e) {
      error = String(e);
    }
  }

  function cancelVersionSelector(serviceType: string) {
    showVersionSelector = { ...showVersionSelector, [serviceType]: false };
  }

  // === Instance Functions ===
  async function createInstance(name: string, port: number, serviceType: string, version: string, config: Record<string, string>, customDomain?: string | null) {
    try {
      error = null;
      await invoke("create_instance", {
        name,
        port,
        serviceType,
        version,
        config: Object.keys(config).length > 0 ? config : null,
        customDomain: customDomain || null
      });
      await loadData();
    } catch (e) {
      error = String(e);
      throw e;
    }
  }

  async function startInstance(id: string) {
    try {
      actionLoading = { ...actionLoading, [id]: true };
      error = null;
      await invoke("start_instance", { id });
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      actionLoading = { ...actionLoading, [id]: false };
    }
  }

  async function stopInstance(id: string) {
    try {
      actionLoading = { ...actionLoading, [id]: true };
      error = null;
      await invoke("stop_instance", { id });
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      actionLoading = { ...actionLoading, [id]: false };
    }
  }

  async function restartInstance(id: string) {
    try {
      actionLoading = { ...actionLoading, [id]: true };
      error = null;
      await invoke("restart_instance", { id });
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      actionLoading = { ...actionLoading, [id]: false };
    }
  }

  async function initializeNoderedInstance(id: string) {
    try {
      initializingInstances = { ...initializingInstances, [id]: true };
      error = null;
      await invoke("init_nodered_instance", { id });
      initializedInstances = { ...initializedInstances, [id]: true };
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      initializingInstances = { ...initializingInstances, [id]: false };
    }
  }

  async function deleteInstance(id: string, name: string) {
    const confirmed = await confirm(
      `Delete instance "${name}"? This will stop the service but keep data.`,
      { title: "Delete Instance", kind: "warning" }
    );
    if (!confirmed) return;
    try {
      actionLoading = { ...actionLoading, [id]: true };
      error = null;
      await invoke("delete_instance", { id });
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      actionLoading = { ...actionLoading, [id]: false };
    }
  }

  function openService(port: number, _serviceType: string) {
    window.open(`http://127.0.0.1:${port}`, "_blank");
  }

  function getDomainUrl(instance: Instance): string {
    if (!networkStatus || !instance.domain_enabled) return "";
    if (proxyStatus?.daemon_installed && proxyStatus?.daemon_running) {
      return `http://${instance.domain}`;
    }
    return `http://${instance.domain}:${networkStatus.proxy_port}`;
  }

  function openDomain(instance: Instance) {
    const url = getDomainUrl(instance);
    if (url) window.open(url, "_blank");
  }

  async function viewLogs(id: string, name: string) {
    try {
      logsLoading = true;
      logsInstanceName = name;
      showLogs = true;
      logsContent = await invoke<string>("get_instance_logs", { id });
    } catch (e) {
      logsContent = `Error loading logs: ${e}`;
    } finally {
      logsLoading = false;
    }
  }

  async function refreshLogs(id: string) {
    try {
      logsLoading = true;
      logsContent = await invoke<string>("get_instance_logs", { id });
    } catch (e) {
      logsContent = `Error loading logs: ${e}`;
    } finally {
      logsLoading = false;
    }
  }

  async function viewEnv(id: string, name: string, serviceType: string) {
    try {
      envLoading = true;
      envInstanceName = name;
      envServiceType = serviceType;
      envCopied = false;
      showEnv = true;
      envContent = await invoke<string>("get_instance_env", { id });
    } catch (e) {
      envContent = `Error loading ENV: ${e}`;
    } finally {
      envLoading = false;
    }
  }

  async function viewInfo(id: string, name: string, serviceType: string) {
    try {
      infoLoading = true;
      infoInstanceName = name;
      infoServiceType = serviceType;
      infoError = null;
      showInfo = true;
      infoContent = await invoke<InstanceInfo>("get_instance_info", { id });
    } catch (e) {
      infoError = String(e);
      infoContent = null;
    } finally {
      infoLoading = false;
    }
  }

  async function copyInfoValue(value: string) {
    try {
      await navigator.clipboard.writeText(value);
      // Show visual feedback (could enhance with a state variable if desired)
    } catch (e) {
      console.error("Failed to copy:", e);
    }
  }

  async function copyEnvToClipboard() {
    try {
      await navigator.clipboard.writeText(envContent);
      envCopied = true;
      setTimeout(() => { envCopied = false; }, 2000);
    } catch (e) {
      error = `Failed to copy: ${e}`;
    }
  }

  function handleServiceTypeChange(serviceType: string): { port: number } {
    const meta = serviceTypes.find(s => s.id === serviceType);
    return { port: meta?.default_port || 7700 };
  }

  // === Stack Functions ===
  async function createStack(name: string, description: string | null, instanceIds: string[]) {
    try {
      error = null;
      await invoke("create_stack", {
        request: {
          name,
          description,
          instance_ids: instanceIds
        }
      });
      await loadData();
    } catch (e) {
      error = String(e);
    }
  }

  function deleteStack(id: string) {
    const stack = stacks.find(s => s.id === id);
    if (stack) {
      deletingStack = stack;
      showDeleteStackModal = true;
    }
  }

  function handleDeleteStackClose() {
    showDeleteStackModal = false;
    deletingStack = null;
  }

  async function handleDeleteStackConfirm(stackId: string, deleteInstances: boolean) {
    try {
      error = null;
      await invoke("delete_stack", { id: stackId, delete_instances: deleteInstances });
      showDeleteStackModal = false;
      deletingStack = null;
      await loadData();
    } catch (e) {
      error = String(e);
    }
  }

  function exportStack(id: string) {
    const stack = stacks.find(s => s.id === id);
    if (stack) {
      exportingStack = stack;
      showExportStackModal = true;
    }
  }

  function handleExportStackClose() {
    showExportStackModal = false;
    exportingStack = null;
  }

  function importStack() {
    showImportStackModal = true;
  }

  function handleImportStackClose() {
    showImportStackModal = false;
  }

  async function handleImportStackComplete() {
    showImportStackModal = false;
    await loadData();
  }

  async function startStack(id: string) {
    try {
      error = null;
      const stackInstances = instances.filter(i => i.stack_id === id && !i.running);
      for (const instance of stackInstances) {
        actionLoading = { ...actionLoading, [instance.id]: true };
      }
      // Start all instances in parallel
      await Promise.all(stackInstances.map(instance => invoke("start_instance", { id: instance.id })));
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      const stackInstances = instances.filter(i => i.stack_id === id);
      for (const instance of stackInstances) {
        actionLoading = { ...actionLoading, [instance.id]: false };
      }
    }
  }

  async function stopStack(id: string) {
    try {
      error = null;
      const stackInstances = instances.filter(i => i.stack_id === id && i.running);
      for (const instance of stackInstances) {
        actionLoading = { ...actionLoading, [instance.id]: true };
      }
      // Stop all instances in parallel
      await Promise.all(stackInstances.map(instance => invoke("stop_instance", { id: instance.id })));
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      const stackInstances = instances.filter(i => i.stack_id === id);
      for (const instance of stackInstances) {
        actionLoading = { ...actionLoading, [instance.id]: false };
      }
    }
  }

  // === Instance Settings ===
  async function openInstanceSettings(instance: Instance) {
    try {
      instanceSettingsLoading = true;
      instanceSettingsId = instance.id;
      instanceSettingsName = instance.name;
      instanceSettingsOriginalName = instance.name;
      instanceSettingsVersion = instance.version;
      instanceSettingsOriginalVersion = instance.version;
      showInstanceSettings = true;
      const result = await invoke<InstanceConfigResponse>("get_instance_config", { id: instance.id });
      instanceSettingsServiceType = result.service_type;

      // Fetch installed versions for this service type
      try {
        instanceSettingsInstalledVersions = await invoke<string[]>("get_installed_versions", {
          serviceType: result.service_type
        });
      } catch (e) {
        console.error("Failed to fetch installed versions:", e);
        instanceSettingsInstalledVersions = [instance.version]; // Fallback to current version
      }

      const config: Record<string, string> = {};
      const serviceMeta = serviceTypes.find(s => s.id === result.service_type);
      if (serviceMeta?.config_fields) {
        for (const field of serviceMeta.config_fields) {
          const value = result.config[field.key];
          config[field.key] = value !== undefined && value !== null ? String(value) : (field.default || "");
        }
      }
      instanceSettingsConfig = config;

      // Load domains for this instance
      instanceSettingsDomains = instance.mapped_domains;
      const allDomains = await invoke<any[]>("list_domains");
      instanceSettingsAllDomains = allDomains.filter(d =>
        instance.mapped_domains.includes(d.full_domain)
      );
    } catch (e) {
      error = String(e);
      showInstanceSettings = false;
    } finally {
      instanceSettingsLoading = false;
    }
  }

  async function saveInstanceSettings() {
    try {
      instanceSettingsSaving = true;
      error = null;

      const instance = instances.find(i => i.id === instanceSettingsId);

      // Change version if different
      if (instanceSettingsVersion !== instanceSettingsOriginalVersion) {
        // Stop instance first if running
        if (instance?.running) {
          await invoke("stop_instance", { id: instanceSettingsId });
        }

        await invoke("change_instance_version", {
          id: instanceSettingsId,
          newVersion: instanceSettingsVersion
        });
      }

      // Rename if name changed
      if (instanceSettingsName.trim() !== instanceSettingsOriginalName) {
        await invoke("rename_instance", { id: instanceSettingsId, newName: instanceSettingsName.trim() });
      }

      await invoke("update_instance_config", { id: instanceSettingsId, config: instanceSettingsConfig });

      // Clear domain form state
      clearDomainForm();
      showInstanceSettings = false;

      if (instance?.running) {
        await message("Settings saved! Restart the instance for changes to take effect.", {
          title: "Settings Saved",
          kind: "info"
        });
      }

      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      instanceSettingsSaving = false;
    }
  }

  // === Instance Settings - Domain Management Functions ===
  async function createDomainForInstance() {
    try {
      const subdomain = newDomainSubdomain.trim().toLowerCase();
      if (!subdomain) {
        await message("Please enter a subdomain", { title: "Invalid Input", kind: "error" });
        return;
      }

      const request = {
        subdomain: subdomain,
        target_type: "instance",
        target_value: instanceSettingsId,
        ssl_enabled: newDomainSsl
      };

      await invoke("create_domain", { request });
      await refreshInstanceSettings();
      clearDomainForm();
    } catch (e) {
      await message(String(e), { title: "Error Creating Domain", kind: "error" });
    }
  }

  async function deleteDomainFromInstance(domainName: string) {
    try {
      const domain = instanceSettingsAllDomains.find(d => d.full_domain === domainName);
      if (!domain) return;

      const confirmed = await confirm(`Delete ${domainName}?`, {
        title: "Delete Domain",
        kind: "warning"
      });
      if (!confirmed) return;

      await invoke("delete_domain", { id: domain.id });
      await refreshInstanceSettings();
    } catch (e) {
      await message(String(e), { title: "Error Deleting Domain", kind: "error" });
    }
  }

  async function toggleDomainSsl(domainName: string) {
    try {
      const domain = instanceSettingsAllDomains.find(d => d.full_domain === domainName);
      if (!domain) return;

      await invoke("update_domain_ssl", {
        id: domain.id,
        sslEnabled: !domain.ssl_enabled
      });
      await refreshInstanceSettings();
    } catch (e) {
      await message(String(e), { title: "Error Toggling SSL", kind: "error" });
    }
  }

  async function refreshInstanceSettings() {
    try {
      // Reload instance data
      const allInstances = await invoke<Instance[]>("list_instances");
      const instance = allInstances.find(i => i.id === instanceSettingsId);
      if (!instance) return;

      instanceSettingsDomains = instance.mapped_domains;

      // Reload all domains to get IDs
      const allDomains = await invoke<any[]>("list_domains");
      instanceSettingsAllDomains = allDomains.filter(d =>
        instance.mapped_domains.includes(d.full_domain)
      );
    } catch (e) {
      console.error("Failed to refresh instance settings:", e);
    }
  }

  function clearDomainForm() {
    showDomainForm = false;
    newDomainSubdomain = "";
    newDomainSsl = true;
  }

  async function browseFolder(fieldKey: string) {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select Folder" });
      if (selected && typeof selected === "string") {
        instanceSettingsConfig = { ...instanceSettingsConfig, [fieldKey]: selected };
      }
    } catch (e) {
      error = String(e);
    }
  }

  function getInstanceServiceMeta(): ServiceInfo | undefined {
    return serviceTypes.find(s => s.id === instanceSettingsServiceType);
  }

  // === Network/DNS Functions ===
  async function installResolver() {
    try {
      installingResolver = true;
      error = null;
      await invoke("install_resolver");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      installingResolver = false;
    }
  }

  async function uninstallResolver() {
    const confirmed = await confirm(
      "Uninstall resolver? Domain routing will stop working.",
      { title: "Uninstall Resolver", kind: "warning" }
    );
    if (!confirmed) return;
    try {
      installingResolver = true;
      error = null;
      await invoke("uninstall_resolver");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      installingResolver = false;
    }
  }

  async function startDnsServer() {
    try {
      dnsServerAction = true;
      error = null;
      await invoke("start_dns_server");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      dnsServerAction = false;
    }
  }

  async function stopDnsServer() {
    try {
      dnsServerAction = true;
      error = null;
      await invoke("stop_dns_server");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      dnsServerAction = false;
    }
  }

  async function restartDnsServer() {
    try {
      dnsServerAction = true;
      error = null;
      await invoke("restart_dns_server");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      dnsServerAction = false;
    }
  }

  // === HTTPS Proxy Functions ===
  async function setupProxy() {
    try {
      settingUpProxy = true;
      error = null;
      await invoke("setup_proxy");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      settingUpProxy = false;
    }
  }

  async function disableProxy() {
    const confirmed = await confirm(
      "Disable reverse proxy? Services will no longer be accessible via domain names.",
      { title: "Disable Proxy", kind: "warning" }
    );
    if (!confirmed) return;
    try {
      disablingProxy = true;
      error = null;
      await invoke("disable_proxy");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      disablingProxy = false;
    }
  }

  async function startDaemon() {
    try {
      startingDaemon = true;
      error = null;
      await invoke("start_proxy_daemon");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      startingDaemon = false;
    }
  }

  async function restartDaemon() {
    try {
      restartingDaemon = true;
      error = null;
      await invoke("restart_proxy_daemon");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      restartingDaemon = false;
    }
  }

  // === CLI Functions ===
  async function installCli() {
    try {
      installingCli = true;
      error = null;
      await invoke("install_cli");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      installingCli = false;
    }
  }

  async function uninstallCli() {
    try {
      installingCli = true;
      error = null;
      await invoke("uninstall_cli");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      installingCli = false;
    }
  }

  // === Helper Functions ===
  async function installHelper() {
    try {
      installingHelper = true;
      error = null;
      await invoke("install_helper");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      installingHelper = false;
    }
  }

  async function uninstallHelper() {
    const confirmed = await confirm(
      "Uninstall the privileged helper? Password prompts will appear again for service operations.",
      { title: "Uninstall Helper", kind: "warning" }
    );
    if (!confirmed) return;
    try {
      installingHelper = true;
      error = null;
      await invoke("uninstall_helper");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      installingHelper = false;
    }
  }

  // === CA Trust Functions ===
  async function trustCA() {
    try {
      trustingCA = true;
      error = null;
      await invoke("trust_caddy_ca");
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      trustingCA = false;
    }
  }

  // === Settings Functions ===
  function openSettings() {
    settingsTld = networkStatus?.tld || "burd";
    showSettings = true;
  }

  async function saveSettings() {
    if (!settingsTld.trim()) {
      error = "TLD cannot be empty";
      return;
    }
    try {
      savingSettings = true;
      error = null;
      await invoke("update_tld", { tld: settingsTld.trim() });
      showSettings = false;
      await loadData();
    } catch (e) {
      error = String(e);
    } finally {
      savingSettings = false;
    }
  }

  // === Navigation ===
  function handleNavigate(section: string) {
    activeSection = section;
  }

  // === Mail Unread Count ===
  $effect(() => {
    if (mailpitExists) {
      invoke<number>("get_unread_count").then((count) => {
        unreadMailCount = count;
      }).catch(() => {
        unreadMailCount = 0;
      });
    } else {
      unreadMailCount = 0;
    }
  });

  // === Lifecycle ===
  onMount(() => {
    loadData();
    const unlistenPromise = listen<DownloadProgress>("download-progress", (event) => {
      downloadProgress = { ...downloadProgress, [event.payload.service_type]: event.payload };
    });
    const interval = setInterval(loadData, 10000);

    // Easter egg: Konami Code reveals The Burd Nest
    const konamiListener = useKonamiCode(() => {
      showBurdNest = true;
    });

    return () => {
      clearInterval(interval);
      unlistenPromise.then((unlisten) => unlisten());
      konamiListener.destroy();
    };
  });
</script>

<div class="app-layout">
  <Sidebar bind:activeSection onNavigate={handleNavigate} {parkEnabled} {mailpitExists} {unreadMailCount} {frpcInstanceExists} />

  <main class="main-content">
    {#if error}
      <div class="error">
        <span>{error}</span>
        <button class="close" onclick={() => (error = null)}>&times;</button>
      </div>
    {/if}

    {#if activeSection === "general"}
      <GeneralSection
        {networkStatus}
        {proxyStatus}
        {cliStatus}
        {helperStatus}
        {dnsServerAction}
        {installingResolver}
        {settingUpProxy}
        {disablingProxy}
        {startingDaemon}
        {restartingDaemon}
        {installingCli}
        {installingHelper}
        onStartDns={startDnsServer}
        onStopDns={stopDnsServer}
        onRestartDns={restartDnsServer}
        onInstallResolver={installResolver}
        onUninstallResolver={uninstallResolver}
        onSetupProxy={setupProxy}
        onDisableProxy={disableProxy}
        onStartDaemon={startDaemon}
        onRestartDaemon={restartDaemon}
        onOpenSettings={openSettings}
        onInstallCli={installCli}
        onUninstallCli={uninstallCli}
        onInstallHelper={installHelper}
        onUninstallHelper={uninstallHelper}
        onRefresh={loadData}
        {caTrustStatus}
        {trustingCA}
        onTrustCA={trustCA}
      />
    {:else if activeSection === "domains"}
      <DomainsSection
        {instances}
        tld={networkStatus?.tld || "burd"}
        onRefresh={loadData}
      />
    {:else if activeSection === "parks"}
      <ParksSection
        tld={networkStatus?.tld || "burd"}
        onRefresh={loadData}
      />
    {:else if activeSection === "tunnels"}
      <TunnelsSection
        {instances}
        onNavigateToServices={() => { activeSection = "services"; }}
        onRefresh={loadData}
        onStartFrpc={startInstance}
      />
    {:else if activeSection === "instances"}
      <InstancesSection
        {instances}
        {stacks}
        {serviceTypes}
        {binaryStatuses}
        {installedVersions}
        {loading}
        resolverInstalled={networkStatus?.resolver_installed || false}
        tld={networkStatus?.tld || "burd"}
        {actionLoading}
        {initializingInstances}
        {initializedInstances}
        onStart={startInstance}
        onStop={stopInstance}
        onRestart={restartInstance}
        onDelete={deleteInstance}
        onCreate={createInstance}
        onViewLogs={viewLogs}
        onViewEnv={viewEnv}
        onViewInfo={viewInfo}
        onOpenSettings={openInstanceSettings}
        onOpenService={openService}
        onOpenDomain={openDomain}
        onServiceTypeChange={handleServiceTypeChange}
        onRefresh={loadData}
        onInitialize={initializeNoderedInstance}
        onCreateStack={createStack}
        onDeleteStack={deleteStack}
        onExportStack={exportStack}
        onImportStack={importStack}
        onStartStack={startStack}
        onStopStack={stopStack}
      />
    {:else if activeSection === "services"}
      <ServicesSection
        {serviceTypes}
        {installedVersions}
        {downloading}
        {downloadProgress}
        {availableVersions}
        {selectedVersions}
        {loadingVersions}
        {showVersionSelector}
        onFetchVersions={fetchVersions}
        onDownload={downloadBinary}
        onCancelVersionSelector={cancelVersionSelector}
        onDeleteVersion={deleteVersion}
      />
    {:else if activeSection === "node"}
      <NodeSection />
    {:else if activeSection === "php"}
      <PHPSection />
    {:else if activeSection === "logs"}
      <LogsSection />
    {:else if activeSection === "tinker"}
      <TinkerSection onRefresh={loadData} />
    {:else if activeSection === "mail"}
      <MailSection onRefresh={loadData} />
    {/if}
  </main>
</div>

<!-- Logs Modal -->
{#if showLogs}
  <div class="modal-overlay" onclick={() => (showLogs = false)} onkeydown={(e) => e.key === 'Escape' && (showLogs = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Logs: {logsInstanceName}</h3>
        <button class="close" onclick={() => (showLogs = false)}>&times;</button>
      </div>
      <div class="modal-body">
        {#if logsLoading}
          <p class="loading">Loading logs...</p>
        {:else}
          <pre class="logs-content">{logsContent}</pre>
        {/if}
      </div>
      <div class="modal-footer">
        <button
          class="btn small secondary"
          onclick={() => {
            const inst = instances.find(i => i.name === logsInstanceName);
            if (inst) refreshLogs(inst.id);
          }}
          disabled={logsLoading}
        >
          Refresh Logs
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- ENV Modal -->
{#if showEnv}
  <div class="modal-overlay" onclick={() => (showEnv = false)} onkeydown={(e) => e.key === 'Escape' && (showEnv = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <div class="modal-title-row">
          <h3>ENV Variables</h3>
          <span class="modal-badge">{envServiceType}</span>
        </div>
        <button class="close" onclick={() => (showEnv = false)}>&times;</button>
      </div>
      <div class="modal-body">
        {#if envLoading}
          <p class="loading">Loading ENV variables...</p>
        {:else}
          <pre class="env-content">{envContent}</pre>
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn secondary" onclick={() => (showEnv = false)}>
          Close
        </button>
        <button
          class="btn primary"
          onclick={copyEnvToClipboard}
          disabled={envLoading}
        >
          {envCopied ? "Copied!" : "Copy to Clipboard"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Instance Info Modal -->
{#if showInfo}
  <div class="modal-overlay" onclick={() => (showInfo = false)} onkeydown={(e) => e.key === 'Escape' && (showInfo = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal info-modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <div class="modal-title-row">
          <h3>{infoInstanceName} - Information</h3>
          <span class="modal-badge">{infoServiceType}</span>
          <span class="modal-badge {infoContent?.running ? 'success' : 'warning'}">
            {infoContent?.running ? 'Running' : 'Stopped'}
          </span>
        </div>
        <button class="close" onclick={() => (showInfo = false)}>&times;</button>
      </div>

      <div class="modal-body">
        {#if infoLoading}
          <p class="loading">Loading information...</p>
        {:else if infoError}
          <div class="error-message">
            <strong>Error:</strong> {infoError}
          </div>
        {:else if infoContent}
          {#each infoContent.categories as category}
            <div class="info-category">
              <h4 class="category-title">{category.title}</h4>
              <div class="info-grid">
                {#each category.items as item}
                  <div class="info-item">
                    <span class="info-label">{item.label}</span>
                    <div class="info-value-row">
                      <span class="info-value">{item.value}</span>
                      {#if item.copyable}
                        <button
                          class="copy-btn"
                          onclick={() => copyInfoValue(item.value)}
                          title="Copy"
                        >
                          <svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
                            <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
                          </svg>
                        </button>
                      {/if}
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/each}
        {/if}
      </div>

      <div class="modal-footer">
        <button class="btn secondary" onclick={() => (showInfo = false)}>
          Close
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Settings Modal -->
{#if showSettings}
  <div class="modal-overlay" onclick={() => (showSettings = false)} onkeydown={(e) => e.key === 'Escape' && (showSettings = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal settings-modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Settings</h3>
        <button class="close" onclick={() => (showSettings = false)}>&times;</button>
      </div>
      <div class="modal-body">
        <form onsubmit={(e) => { e.preventDefault(); saveSettings(); }}>
          <div class="settings-group">
            <label>
              <span class="settings-label">Domain TLD</span>
              <div class="tld-input-group">
                <span class="tld-prefix">.</span>
                <input
                  type="text"
                  bind:value={settingsTld}
                  placeholder="burd"
                  pattern="[a-zA-Z][a-zA-Z0-9]*"
                  required
                />
              </div>
              <span class="settings-hint">Services will be available at <code>*.{settingsTld || 'burd'}</code></span>
            </label>
          </div>
          <button class="btn primary" type="submit" disabled={savingSettings}>
            {savingSettings ? "Saving..." : "Save Settings"}
          </button>
        </form>
      </div>
    </div>
  </div>
{/if}

<!-- Instance Settings Modal -->
{#if showInstanceSettings}
  <div class="modal-overlay" onclick={() => (showInstanceSettings = false)} onkeydown={(e) => e.key === 'Escape' && (showInstanceSettings = false)} role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal settings-modal" onclick={(e) => e.stopPropagation()} role="document">
      <div class="modal-header">
        <h3>Settings: {instanceSettingsName}</h3>
        <button class="close" onclick={() => (showInstanceSettings = false)}>&times;</button>
      </div>
      <div class="modal-body">
        {#if instanceSettingsLoading}
          <p class="loading">Loading configuration...</p>
        {:else}
          <div class="settings-group">
            <label>
              <span class="settings-label">Name</span>
              <input
                type="text"
                bind:value={instanceSettingsName}
                placeholder="Instance name..."
              />
            </label>
          </div>

          <!-- Version Selector -->
          <div class="settings-group">
            <label>
              <span class="settings-label">Version</span>
              <select bind:value={instanceSettingsVersion}>
                {#each instanceSettingsInstalledVersions as ver}
                  <option value={ver}>
                    {ver}{ver === instanceSettingsOriginalVersion ? " (current)" : ""}
                  </option>
                {/each}
              </select>
            </label>
            {#if instanceSettingsVersion !== instanceSettingsOriginalVersion}
              <p class="version-warning" style="margin-top: 0.5rem; font-size: 0.85rem; color: #ff9800;">
                ⚠️ Changing version requires restart. Data compatibility not guaranteed.
              </p>
            {/if}
          </div>

          {@const serviceMeta = getInstanceServiceMeta()}
          {#if serviceMeta?.config_fields && serviceMeta.config_fields.length > 0}
            <form onsubmit={(e) => { e.preventDefault(); saveInstanceSettings(); }}>
              {#each serviceMeta.config_fields as field (field.key)}
                <div class="settings-group">
                  <label>
                    <span class="settings-label">
                      {field.label}
                      {#if field.required}
                        <span class="required">*</span>
                      {/if}
                    </span>
                    {#if field.field_type === "folder"}
                      <div class="folder-input-group">
                        <input
                          type="text"
                          value={instanceSettingsConfig[field.key] || ""}
                          oninput={(e) => { instanceSettingsConfig = { ...instanceSettingsConfig, [field.key]: e.currentTarget.value }; }}
                          placeholder={field.default || "Select a folder..."}
                          class="folder-input"
                        />
                        <button type="button" class="btn secondary small browse-btn" onclick={() => browseFolder(field.key)}>
                          Browse
                        </button>
                      </div>
                    {:else if field.field_type === "password"}
                      <input
                        type="password"
                        value={instanceSettingsConfig[field.key] || ""}
                        oninput={(e) => { instanceSettingsConfig = { ...instanceSettingsConfig, [field.key]: e.currentTarget.value }; }}
                        placeholder={field.default ? "(using default)" : "Enter value..."}
                      />
                    {:else}
                      <input
                        type="text"
                        value={instanceSettingsConfig[field.key] || ""}
                        oninput={(e) => { instanceSettingsConfig = { ...instanceSettingsConfig, [field.key]: e.currentTarget.value }; }}
                        placeholder={field.default || "Enter value..."}
                      />
                    {/if}
                    {#if field.default}
                      <span class="settings-hint">Default: {field.default}</span>
                    {/if}
                  </label>
                </div>
              {/each}
            </form>
          {:else}
            <p class="empty">No configurable settings for this service type.</p>
          {/if}

          <!-- Domains Section -->
          {#if !instanceSettingsLoading}
            <div class="settings-section" style="margin-top: 2rem; padding-top: 1.5rem; border-top: 1px solid var(--border);">
              <h4 style="margin-bottom: 1rem; font-size: 1rem; font-weight: 600;">Domains</h4>

              {#if instanceSettingsDomains.length > 0}
                <div class="domains-list" style="margin-bottom: 1rem;">
                  {#each instanceSettingsDomains as domainName}
                    {@const domain = instanceSettingsAllDomains.find(d => d.full_domain === domainName)}
                    <div class="domain-row" style="display: flex; align-items: center; justify-content: space-between; padding: 0.75rem; background: var(--bg-secondary); border-radius: 6px; margin-bottom: 0.5rem;">
                      <div style="display: flex; align-items: center; gap: 0.75rem;">
                        <span style="font-family: monospace; font-size: 0.9rem;">{domainName}</span>
                        {#if domain?.ssl_enabled}
                          <span class="ssl-badge" style="padding: 0.25rem 0.5rem; background: #4caf50; color: white; font-size: 0.75rem; border-radius: 4px; font-weight: 500;">SSL</span>
                        {/if}
                      </div>
                      <div style="display: flex; gap: 0.5rem;">
                        <button
                          class="btn secondary small"
                          onclick={() => toggleDomainSsl(domainName)}
                          title={domain?.ssl_enabled ? "Disable SSL" : "Enable SSL"}
                          style="padding: 0.25rem 0.5rem; font-size: 0.8rem;"
                        >
                          {domain?.ssl_enabled ? "🔒" : "🔓"}
                        </button>
                        <button
                          class="btn danger small"
                          onclick={() => deleteDomainFromInstance(domainName)}
                          title="Delete domain"
                          style="padding: 0.25rem 0.5rem; font-size: 0.8rem;"
                        >
                          🗑️
                        </button>
                      </div>
                    </div>
                  {/each}
                </div>
              {:else}
                <p class="empty" style="margin-bottom: 1rem; font-size: 0.9rem; color: var(--text-secondary);">No domains attached</p>
              {/if}

              {#if !showDomainForm}
                <button
                  class="btn secondary small"
                  onclick={() => showDomainForm = true}
                  style="width: 100%;"
                >
                  + Add Domain
                </button>
              {:else}
                <div class="domain-form" style="padding: 1rem; background: var(--bg-secondary); border-radius: 6px;">
                  <div style="margin-bottom: 0.75rem;">
                    <label style="display: block; margin-bottom: 0.25rem; font-size: 0.9rem; font-weight: 500;">Subdomain</label>
                    <div style="display: flex; align-items: center; gap: 0.5rem;">
                      <input
                        type="text"
                        bind:value={newDomainSubdomain}
                        placeholder="subdomain"
                        style="flex: 1; font-family: monospace;"
                      />
                      <span style="color: var(--text-secondary);">.{networkStatus?.tld || 'burd'}</span>
                    </div>
                  </div>
                  <div style="margin-bottom: 1rem;">
                    <label style="display: flex; align-items: center; gap: 0.5rem; cursor: pointer;">
                      <input type="checkbox" bind:checked={newDomainSsl} />
                      <span style="font-size: 0.9rem;">SSL Enabled</span>
                    </label>
                  </div>
                  <div style="display: flex; gap: 0.5rem;">
                    <button
                      class="btn primary small"
                      onclick={createDomainForInstance}
                      style="flex: 1;"
                    >
                      Create
                    </button>
                    <button
                      class="btn secondary small"
                      onclick={clearDomainForm}
                      style="flex: 1;"
                    >
                      Cancel
                    </button>
                  </div>
                </div>
              {/if}
            </div>
          {/if}
        {/if}
      </div>
      <div class="modal-footer">
        <button class="btn secondary" onclick={() => (showInstanceSettings = false)}>
          Cancel
        </button>
        <button
          class="btn primary"
          onclick={saveInstanceSettings}
          disabled={instanceSettingsSaving || instanceSettingsLoading}
        >
          {instanceSettingsSaving ? "Saving..." : "Save Settings"}
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Easter Egg: The Burd Nest Terminal -->
<BurdNest show={showBurdNest} onClose={() => (showBurdNest = false)} />

<!-- Import Stack Modal -->
<ImportStackModal
  show={showImportStackModal}
  onClose={handleImportStackClose}
  onImported={handleImportStackComplete}
/>

<!-- Export Stack Modal -->
<ExportStackModal
  show={showExportStackModal}
  stack={exportingStack}
  onClose={handleExportStackClose}
/>

<!-- Delete Stack Modal -->
<DeleteStackModal
  show={showDeleteStackModal}
  stack={deletingStack}
  {instances}
  onClose={handleDeleteStackClose}
  onConfirm={handleDeleteStackConfirm}
/>

<style>
  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif;
    background: #f5f5f7;
    color: #1d1d1f;
  }

  @media (prefers-color-scheme: dark) {
    :global(body) {
      background: #1c1c1e;
      color: #f5f5f7;
    }
  }

  /* Explicit theme overrides via data-theme attribute */
  :global(:root[data-theme="light"]) {
    color-scheme: light;
  }

  :global(:root[data-theme="dark"]) {
    color-scheme: dark;
  }

  :global(:root[data-theme="light"] body) {
    background: #f5f5f7 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="dark"] body) {
    background: #1c1c1e !important;
    color: #f5f5f7 !important;
  }

  .app-layout {
    display: flex;
    min-height: 100vh;
  }

  .main-content {
    flex: 1;
    padding: 2rem;
    overflow-y: auto;
    max-width: 1000px;
  }

  .error {
    background: #ff3b30;
    color: white;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1.5rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .error .close {
    background: none;
    border: none;
    color: white;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0;
    line-height: 1;
  }

  @media (prefers-color-scheme: dark) {
    .error {
      background: #dc2626;
    }
  }

  /* Button styles */
  .btn {
    padding: 0.5rem 1rem;
    border: none;
    border-radius: 6px;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .btn.small {
    padding: 0.375rem 0.75rem;
    font-size: 0.8125rem;
  }

  .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
  }

  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn.secondary {
    background: #e5e5e5;
    color: #1d1d1f;
  }

  .btn.secondary:hover:not(:disabled) {
    background: #d1d1d6;
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  @media (prefers-color-scheme: dark) {
    .btn.secondary {
      background: #3a3a3c;
      color: #f5f5f7;
    }

    .btn.secondary:hover:not(:disabled) {
      background: #48484a;
    }
  }

  /* Modal styles */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: white;
    border-radius: 12px;
    width: 90%;
    max-width: 600px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 40px rgba(0, 0, 0, 0.2);
  }

  @media (prefers-color-scheme: dark) {
    .modal {
      background: #2c2c2e;
    }
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem 1.5rem;
    border-bottom: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .modal-header {
      border-bottom-color: #38383a;
    }
  }

  .modal-header h3 {
    margin: 0;
    font-size: 1.125rem;
    font-weight: 600;
  }

  .modal-header .close {
    background: none;
    border: none;
    font-size: 1.5rem;
    cursor: pointer;
    color: #86868b;
    padding: 0;
    line-height: 1;
  }

  .modal-header .close:hover {
    color: #1d1d1f;
  }

  @media (prefers-color-scheme: dark) {
    .modal-header .close:hover {
      color: #f5f5f7;
    }
  }

  .modal-body {
    padding: 1.5rem;
    overflow-y: auto;
    flex: 1;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    padding: 1rem 1.5rem;
    border-top: 1px solid #e5e5e5;
  }

  @media (prefers-color-scheme: dark) {
    .modal-footer {
      border-top-color: #38383a;
    }
  }

  .logs-content {
    background: #1c1c1e;
    color: #f5f5f7;
    padding: 1rem;
    border-radius: 8px;
    font-family: monospace;
    font-size: 0.75rem;
    line-height: 1.5;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
    max-height: 400px;
    overflow-y: auto;
  }

  .env-content {
    background: #1c1c1e;
    color: #a8e6cf;
    padding: 1rem;
    border-radius: 8px;
    font-family: monospace;
    font-size: 0.8125rem;
    line-height: 1.6;
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
    margin: 0;
    max-height: 400px;
    overflow-y: auto;
  }

  .modal-title-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .modal-title-row h3 {
    margin: 0;
  }

  .modal-badge {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
    font-size: 0.6875rem;
    font-weight: 600;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    text-transform: uppercase;
  }

  .modal-badge.success {
    background: linear-gradient(135deg, #16a34a, #15803d);
  }

  .modal-badge.warning {
    background: linear-gradient(135deg, #f59e0b, #d97706);
  }

  /* Info Modal Styles */
  .info-modal {
    max-width: 700px;
    max-height: 80vh;
  }

  .info-category {
    margin-bottom: 1.5rem;
  }

  .info-category:last-child {
    margin-bottom: 0;
  }

  .category-title {
    font-size: 0.75rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: #86868b;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .info-grid {
    display: grid;
    gap: 0.75rem;
  }

  .info-item {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 1rem;
    padding: 0.625rem 0.75rem;
    background: #f5f5f7;
    border-radius: 6px;
    align-items: center;
  }

  @media (prefers-color-scheme: dark) {
    .info-item {
      background: #2c2c2e;
    }
  }

  .info-label {
    font-weight: 500;
    color: #86868b;
    font-size: 0.875rem;
  }

  .info-value-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .info-value {
    font-family: 'SF Mono', 'Monaco', 'Menlo', monospace;
    font-size: 0.8125rem;
    word-break: break-all;
    flex: 1;
    min-width: 0;
  }

  .copy-btn {
    padding: 4px;
    background: transparent;
    border: none;
    cursor: pointer;
    opacity: 0.6;
    color: #007aff;
    transition: opacity 0.15s ease;
    flex-shrink: 0;
  }

  .copy-btn:hover {
    opacity: 1;
  }

  @media (prefers-color-scheme: dark) {
    .copy-btn {
      color: #0a84ff;
    }
  }

  .error-message {
    background: #ff3b30;
    color: white;
    padding: 1rem;
    border-radius: 8px;
  }

  @media (prefers-color-scheme: dark) {
    .error-message {
      background: #dc2626;
    }
  }

  /* Settings form styles */
  .settings-group {
    margin-bottom: 1.5rem;
  }

  .settings-group label {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .settings-label {
    font-weight: 500;
    font-size: 0.875rem;
  }

  .settings-label .required {
    color: #ff3b30;
  }

  .settings-hint {
    font-size: 0.75rem;
    color: #86868b;
  }

  .settings-hint code {
    background: #f5f5f7;
    padding: 0.125rem 0.25rem;
    border-radius: 3px;
    font-size: 0.6875rem;
  }

  @media (prefers-color-scheme: dark) {
    .settings-hint code {
      background: #1c1c1e;
    }
  }

  .tld-input-group {
    display: flex;
    align-items: center;
  }

  .tld-prefix {
    padding: 0.5rem;
    background: #e5e5e5;
    border: 1px solid #d1d1d6;
    border-right: none;
    border-radius: 6px 0 0 6px;
    font-size: 0.875rem;
    color: #86868b;
  }

  @media (prefers-color-scheme: dark) {
    .tld-prefix {
      background: #3a3a3c;
      border-color: #48484a;
    }
  }

  .tld-input-group input {
    flex: 1;
    border-radius: 0 6px 6px 0;
  }

  .folder-input-group {
    display: flex;
    gap: 0.5rem;
  }

  .folder-input {
    flex: 1;
  }

  input {
    padding: 0.5rem;
    border: 1px solid #d1d1d6;
    border-radius: 6px;
    font-size: 0.875rem;
    background: white;
    color: inherit;
  }

  @media (prefers-color-scheme: dark) {
    input {
      background: #1c1c1e;
      border-color: #3a3a3c;
    }
  }

  input:focus {
    outline: none;
    border-color: #007aff;
  }

  .loading, .empty {
    text-align: center;
    color: #86868b;
    padding: 2rem;
  }

  /* Explicit dark theme overrides via data-theme attribute */
  :global(:root[data-theme="dark"] .card) {
    background: #2c2c2e !important;
  }

  :global(:root[data-theme="dark"] footer) {
    border-top-color: #38383a;
  }

  :global(:root[data-theme="dark"] .btn.secondary) {
    background: #3a3a3c;
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"] .btn.secondary:hover:not(:disabled)) {
    background: #48484a;
  }

  :global(:root[data-theme="dark"] .modal) {
    background: #2c2c2e;
  }

  :global(:root[data-theme="dark"] .modal-header) {
    border-bottom-color: #38383a;
  }

  :global(:root[data-theme="dark"] .modal-header .close:hover) {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"] .modal-footer) {
    border-top-color: #38383a;
  }

  :global(:root[data-theme="dark"] .settings-hint code) {
    background: #1c1c1e;
  }

  :global(:root[data-theme="dark"] .tld-prefix) {
    background: #3a3a3c;
    border-color: #48484a;
  }

  :global(:root[data-theme="dark"] input) {
    background: #1c1c1e;
    border-color: #3a3a3c;
  }

  :global(:root[data-theme="dark"] .network-item),
  :global(:root[data-theme="dark"] .instance-item),
  :global(:root[data-theme="dark"] .service-item),
  :global(:root[data-theme="dark"] .domain-item),
  :global(:root[data-theme="dark"] .tunnel-item),
  :global(:root[data-theme="dark"] .server-item),
  :global(:root[data-theme="dark"] .version-item) {
    background: #1c1c1e !important;
  }

  /* Explicit light theme overrides via data-theme attribute */
  :global(:root[data-theme="light"] .card) {
    background: white !important;
  }

  :global(:root[data-theme="light"] .card.warning) {
    background: #fef3c7 !important;
  }

  :global(:root[data-theme="light"] footer) {
    border-top-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .btn.secondary) {
    background: #e5e5e5 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"] .btn.secondary:hover:not(:disabled)) {
    background: #d1d1d6 !important;
  }

  :global(:root[data-theme="light"] .modal) {
    background: white !important;
  }

  :global(:root[data-theme="light"] .modal-header) {
    border-bottom-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .modal-footer) {
    border-top-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .settings-hint code) {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="light"] .tld-prefix) {
    background: #f5f5f7 !important;
    border-color: #d1d1d6 !important;
  }

  :global(:root[data-theme="light"] input),
  :global(:root[data-theme="light"] select),
  :global(:root[data-theme="light"] textarea) {
    background: white !important;
    border-color: #d1d1d6 !important;
    color: #1d1d1f !important;
  }

  /* Common section elements - light mode */
  :global(:root[data-theme="light"] .network-item),
  :global(:root[data-theme="light"] .instance-item),
  :global(:root[data-theme="light"] .service-item),
  :global(:root[data-theme="light"] .domain-item),
  :global(:root[data-theme="light"] .tunnel-item),
  :global(:root[data-theme="light"] .server-item),
  :global(:root[data-theme="light"] .version-item) {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="light"] .form-container),
  :global(:root[data-theme="light"] .network-hint),
  :global(:root[data-theme="light"] .hint-box),
  :global(:root[data-theme="light"] .info-box) {
    background: #f5f5f7 !important;
  }

  :global(:root[data-theme="light"] .small-button) {
    background: #e5e5e5 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"] .status-card) {
    background: white !important;
  }

  :global(:root[data-theme="light"] .status-card.warning) {
    background: #fef3c7 !important;
    border-color: #f59e0b !important;
  }

  :global(:root[data-theme="light"] .status-card.success) {
    background: #dcfce7 !important;
    border-color: #16a34a !important;
  }

  :global(:root[data-theme="light"] .status-card.info) {
    background: #dbeafe !important;
    border-color: #2563eb !important;
  }

  :global(:root[data-theme="light"] .error-banner) {
    background: #fee2e2 !important;
    color: #dc2626 !important;
  }

  :global(:root[data-theme="light"] pre),
  :global(:root[data-theme="light"] code) {
    background: #f5f5f7 !important;
    color: #1d1d1f !important;
  }

  :global(:root[data-theme="light"] .label),
  :global(:root[data-theme="light"] .hint),
  :global(:root[data-theme="light"] .desc) {
    color: #86868b !important;
  }

  :global(:root[data-theme="light"] section.card) {
    background: white !important;
  }

  /* Error banner explicit overrides */
  :global(:root[data-theme="light"]) .error {
    background: #ff3b30 !important;
    color: white !important;
  }

  :global(:root[data-theme="dark"]) .error {
    background: #dc2626 !important;
    color: white !important;
  }

  /* Dark mode additional fixes */
</style>
