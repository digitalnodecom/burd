<script lang="ts">
  import InstanceSettingsModal from "$lib/components/InstanceSettingsModal.svelte";
  import InstanceInspector from "$lib/components/InstanceInspector.svelte";
  import Modal from "$lib/components/Modal.svelte";
  import { deleteStack as removeStack } from "$lib/api/instances";
  import { X } from '@lucide/svelte';
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { confirm, message } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { useKonamiCode } from "$lib/composables/useKonamiCode.svelte";
  import { createUpdater } from "$lib/composables/useUpdater.svelte";

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

  import type { Instance, ServiceInfo, ConfigFieldInfo } from "$lib/types";
  // === Type Definitions ===

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
    proxy_healthy: boolean | null;
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

  // === Navigation State ===
  let activeSection = $state("general");
  let pendingEmailId = $state<string | null>(null);

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

  let inspector = $state<{ id: string; name: string; serviceType: string; mode: 'logs' | 'env' | 'info' } | null>(null);
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

  // Tunnel status (show when frpc binary is downloaded)
  let frpcDownloaded = $derived((installedVersions['frpc']?.length ?? 0) > 0);

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

  let settingsInstance = $state<Instance | null>(null);

  // === Data Loading ===
  let refreshTask: Promise<void> | null = null;
  function loadData(): Promise<void> {
    if (refreshTask) return refreshTask;
    refreshTask = refreshData().finally(() => { refreshTask = null; });
    return refreshTask;
  }
  async function refreshData() {
    try {
      loading = instances.length === 0;
      const results = await Promise.allSettled([
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
      if (results[0].status === "fulfilled") instances = results[0].value;
      if (results[1].status === "fulfilled") stacks = results[1].value;
      if (results[2].status === "fulfilled") binaryStatuses = results[2].value;
      if (results[3].status === "fulfilled") serviceTypes = results[3].value.filter((s: ServiceInfo) => s.available);
      if (results[4].status === "fulfilled") networkStatus = results[4].value;
      if (results[5].status === "fulfilled") proxyStatus = results[5].value;
      if (results[6].status === "fulfilled") cliStatus = results[6].value;
      if (results[7].status === "fulfilled") helperStatus = results[7].value;
      if (results[8].status === "fulfilled") parkEnabled = results[8].value;
      if (results[9].status === "fulfilled") caTrustStatus = results[9].value;
      const failures = results.filter(r => r.status === "rejected");
      if (failures.length) error = failures.map(r => String((r as PromiseRejectedResult).reason)).join("; ");
      // Fetch installed versions for all services
      for (const svc of serviceTypes) {
        try {
          const versions = await invoke<string[]>("get_installed_versions", { serviceType: svc.id });
          installedVersions = { ...installedVersions, [svc.id]: versions };
        } catch {
          installedVersions = { ...installedVersions, [svc.id]: [] };
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

  function viewLogs(id: string, name: string) { inspector = { id, name, serviceType: '', mode: 'logs' }; }
  function viewEnv(id: string, name: string, serviceType: string) { inspector = { id, name, serviceType, mode: 'env' }; }
  function viewInfo(id: string, name: string, serviceType: string) { inspector = { id, name, serviceType, mode: 'info' }; }

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
      await removeStack(stackId, deleteInstances);
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
      const results = await Promise.allSettled(stackInstances.map(instance => invoke("start_instance", { id: instance.id })));
      await loadData();
      const failures = results.flatMap((result, index) => result.status === "rejected" ? [`${stackInstances[index].name}: ${result.reason}`] : []);
      if (failures.length) error = failures.join("; ");
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
      const results = await Promise.allSettled(stackInstances.map(instance => invoke("stop_instance", { id: instance.id })));
      await loadData();
      const failures = results.flatMap((result, index) => result.status === "rejected" ? [`${stackInstances[index].name}: ${result.reason}`] : []);
      if (failures.length) error = failures.join("; ");
    } catch (e) {
      error = String(e);
    } finally {
      const stackInstances = instances.filter(i => i.stack_id === id);
      for (const instance of stackInstances) {
        actionLoading = { ...actionLoading, [instance.id]: false };
      }
    }
  }

  function openInstanceSettings(instance: Instance) { settingsInstance = instance; }

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
  // Refresh the sidebar's unread badge. Called on load, on the `new-email`
  // event, and on the periodic poll so reads/deletes are reflected too.
  function refreshUnreadCount() {
    if (!mailpitExists) {
      unreadMailCount = 0;
      return;
    }
    invoke<number>("get_unread_count")
      .then((count) => { unreadMailCount = count; })
      .catch(() => { unreadMailCount = 0; });
  }

  // Re-fetch on mount, when Mailpit's availability changes, and on navigation
  // (so reading mail and leaving the section clears the badge promptly).
  $effect(() => {
    void mailpitExists;
    void activeSection;
    refreshUnreadCount();
  });

  // === Lifecycle ===
  const updater = createUpdater();

  // Manual "Check for Updates…" (from the macOS app menu): unlike the silent
  // background check, this always reports the outcome to the user.
  async function checkForUpdatesManual() {
    if (updater.checking || updater.downloading) return;
    await updater.checkForUpdate();
    if (updater.error) {
      await message(`Couldn't check for updates.\n\n${updater.error}`, {
        title: "Check for Updates",
        kind: "error",
      });
    } else if (updater.available) {
      const install = await confirm(
        `Burd ${updater.version} is available.\n\nDownload and install it now? Burd will restart.`,
        { title: "Update Available", kind: "info", okLabel: "Install & Restart", cancelLabel: "Later" }
      );
      if (install) await updater.installAndRestart();
    } else {
      await message("Burd is up to date.", { title: "Check for Updates", kind: "info" });
    }
  }

  onMount(() => {
    // Start any instances flagged to auto-start, once, after the first load.
    loadData().then(() => {
      for (const inst of instances) {
        if (inst.auto_start && !inst.running) {
          invoke("start_instance", { id: inst.id }).catch((e) =>
            console.error(`auto-start ${inst.name} failed:`, e)
          );
        }
      }
    });
    // Check for an app update now, then every 30 minutes.
    updater.checkForUpdate();
    updater.startAutoCheck();
    const unlistenPromise = listen<DownloadProgress>("download-progress", (event) => {
      downloadProgress = { ...downloadProgress, [event.payload.service_type]: event.payload };
    });
    const healthUnlistenPromise = listen<boolean | null>("proxy-health-changed", (event) => {
      if (proxyStatus) {
        proxyStatus = { ...proxyStatus, proxy_healthy: event.payload };
      }
    });
    const herdUnlistenPromise = listen<string>("herd-conflict", (event) => {
      message(event.payload, { title: "Conflict Detected", kind: "warning" });
    });
    const trayNavUnlistenPromise = listen<{ section?: string; instanceId?: string }>(
      "tray-navigate",
      (event) => {
        if (event.payload?.section) {
          activeSection = event.payload.section;
        }
      }
    );
    const openEmailUnlistenPromise = listen<string>("open-email", (event) => {
      pendingEmailId = event.payload;
      activeSection = "mail";
    });
    const instancesChangedUnlistenPromise = listen("instances-changed", () => {
      loadData();
    });
    const checkUpdatesUnlistenPromise = listen("menu:check-for-updates", () => {
      void checkForUpdatesManual();
    });
    // A new email bumps the sidebar badge instantly (backend also updates the
    // macOS dock badge on the same websocket event).
    const newEmailUnlistenPromise = listen("new-email", () => {
      refreshUnreadCount();
    });
    const interval = setInterval(() => {
      loadData();
      refreshUnreadCount();
    }, 10000);

    // Easter egg: Konami Code reveals The Burd Nest
    const konamiListener = useKonamiCode(() => {
      showBurdNest = true;
    });

    return () => {
      clearInterval(interval);
      unlistenPromise.then((unlisten) => unlisten());
      healthUnlistenPromise.then((unlisten) => unlisten());
      herdUnlistenPromise.then((unlisten) => unlisten());
      trayNavUnlistenPromise.then((unlisten) => unlisten());
      openEmailUnlistenPromise.then((unlisten) => unlisten());
      instancesChangedUnlistenPromise.then((unlisten) => unlisten());
      checkUpdatesUnlistenPromise.then((unlisten) => unlisten());
      newEmailUnlistenPromise.then((unlisten) => unlisten());
      konamiListener.destroy();
      updater.stopAutoCheck();
    };
  });
</script>

<div class="app-layout">
  <Sidebar bind:activeSection onNavigate={handleNavigate} {parkEnabled} {mailpitExists} {unreadMailCount} {frpcDownloaded} {updater} />

  <main class="main-content">
    {#if error}
      <div class="error">
        <span>{error}</span>
        <button aria-label="Close" class="modal-close" onclick={() => (error = null)}><X size={16} strokeWidth={2} /></button>
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
        onNavigateToServices={() => { activeSection = "services"; }}
        {instances}
        {stacks}
        {serviceTypes}
        {binaryStatuses}
        {installedVersions}
        {loading}
        resolverInstalled={networkStatus?.resolver_installed || false}
        tld={networkStatus?.tld || "burd"}
        {actionLoading}
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
      <MailSection onRefresh={loadData} bind:openEmailId={pendingEmailId} />
    {/if}
  </main>
</div>

{#if inspector}
  {#key inspector.id + inspector.mode}
    <InstanceInspector target={inspector} onClose={() => inspector = null} />
  {/key}
{/if}

<!-- Settings Modal -->
{#if showSettings}
  <Modal onClose={() => (showSettings = false)} overlayClass="modal-overlay">
    <div class="modal settings-modal"  role="document">
      <div class="modal-header">
        <h3 class="modal-title">Settings</h3>
        <button aria-label="Close" class="modal-close" onclick={() => (showSettings = false)}><X size={16} strokeWidth={2} /></button>
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
  </Modal>
{/if}

<!-- Instance Settings Modal -->
{#if settingsInstance}
  {#key settingsInstance.id}
    <InstanceSettingsModal instance={settingsInstance} {instances} {serviceTypes} tld={networkStatus?.tld || 'burd'} onClose={() => settingsInstance = null} onRefresh={loadData} />
  {/key}
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
    background: var(--surface-2);
    color: var(--text);
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
    background: var(--surface-2) !important;
    color: var(--text) !important;
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

  .btn.primary {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
  }

  .btn.primary:hover:not(:disabled) {
    filter: brightness(1.1);
  }

  .btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  @media (prefers-color-scheme: dark) {

  }

  /* Modal styles */

  .modal {
    background: var(--surface);
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

  @media (prefers-color-scheme: dark) {
  }

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  /* Info Modal Styles */

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  @media (prefers-color-scheme: dark) {

  }

  /* Settings form styles */
  .settings-group {
    margin-bottom: 0.85rem;
  }

  /* Name + Port side by side */

  .settings-group label {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }

  .settings-label {
    font-weight: 500;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  /* Auto-start toggle — a plain inline row */

  /* Domains — compact */

  .settings-hint {
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .settings-hint code {
    background: var(--surface-2);
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
    border: 1px solid var(--border-strong);
    border-right: none;
    border-radius: 6px 0 0 6px;
    font-size: 0.875rem;
    color: var(--text-muted);
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

  input {
    padding: 0.5rem;
    border: 1px solid var(--border-strong);
    border-radius: 6px;
    font-size: 0.875rem;
    line-height: 1.4;
    background: var(--surface);
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

  :global(:root[data-theme="dark"] input),
  :global(:root[data-theme="dark"] select) {
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
    background: var(--surface) !important;
  }

  :global(:root[data-theme="light"] .card.warning) {
    background: #fef3c7 !important;
  }

  :global(:root[data-theme="light"] footer) {
    border-top-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .btn.secondary) {
    background: #e5e5e5 !important;
    color: var(--text) !important;
  }

  :global(:root[data-theme="light"] .btn.secondary:hover:not(:disabled)) {
    background: #d1d1d6 !important;
  }

  :global(:root[data-theme="light"] .modal) {
    background: var(--surface) !important;
  }

  :global(:root[data-theme="light"] .modal-header) {
    border-bottom-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .modal-footer) {
    border-top-color: #e5e5e5 !important;
  }

  :global(:root[data-theme="light"] .settings-hint code) {
    background: var(--surface-2) !important;
  }

  :global(:root[data-theme="light"] .tld-prefix) {
    background: var(--surface-2) !important;
    border-color: #d1d1d6 !important;
  }

  :global(:root[data-theme="light"] input),
  :global(:root[data-theme="light"] select),
  :global(:root[data-theme="light"] textarea) {
    background: var(--surface) !important;
    border-color: #d1d1d6 !important;
    color: var(--text) !important;
  }

  /* Common section elements - light mode */
  :global(:root[data-theme="light"] .network-item),
  :global(:root[data-theme="light"] .instance-item),
  :global(:root[data-theme="light"] .service-item),
  :global(:root[data-theme="light"] .domain-item),
  :global(:root[data-theme="light"] .tunnel-item),
  :global(:root[data-theme="light"] .server-item),
  :global(:root[data-theme="light"] .version-item) {
    background: var(--surface-2) !important;
  }

  :global(:root[data-theme="light"] .form-container),
  :global(:root[data-theme="light"] .network-hint),
  :global(:root[data-theme="light"] .hint-box),
  :global(:root[data-theme="light"] .info-box) {
    background: var(--surface-2) !important;
  }

  :global(:root[data-theme="light"] .small-button) {
    background: #e5e5e5 !important;
    color: var(--text) !important;
  }

  :global(:root[data-theme="light"] .status-card) {
    background: var(--surface) !important;
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
    background: var(--surface-2) !important;
    color: var(--text) !important;
  }

  :global(:root[data-theme="light"] .label),
  :global(:root[data-theme="light"] .hint),
  :global(:root[data-theme="light"] .desc) {
    color: var(--text-muted) !important;
  }

  :global(:root[data-theme="light"] section.card) {
    background: var(--surface) !important;
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
