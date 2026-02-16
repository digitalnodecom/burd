/**
 * frpc connection status polling composable
 * Handles periodic polling of frpc connection status
 */

import { invoke } from "@tauri-apps/api/core";

interface FrpcProxyStatus {
  name: string;
  status: string;
  local_addr: string;
  remote_addr: string | null;
  error: string | null;
}

interface FrpcConnectionStatus {
  running: boolean;
  connected: boolean;
  server_addr: string | null;
  error: string | null;
  proxies: FrpcProxyStatus[];
}

interface FrpcInstance {
  id: string;
  running: boolean;
}

interface StatusSummary {
  icon: string;
  label: string;
  detail: string;
  class: string;
}

export function createFrpcPolling(
  getFrpcInstance: () => FrpcInstance | undefined,
  getTunnelCount: () => number
) {
  let connectionStatus = $state<FrpcConnectionStatus | null>(null);
  let pollingInterval: ReturnType<typeof setInterval> | null = null;

  async function loadConnectionStatus() {
    try {
      connectionStatus = await invoke<FrpcConnectionStatus>("get_frpc_connection_status");
    } catch (e) {
      console.error("Failed to get frpc connection status:", e);
      connectionStatus = null;
    }
  }

  function startPolling() {
    if (pollingInterval) return;
    loadConnectionStatus();
    pollingInterval = setInterval(loadConnectionStatus, 3000);
  }

  function stopPolling() {
    if (pollingInterval) {
      clearInterval(pollingInterval);
      pollingInterval = null;
    }
  }

  function getConnectedTunnelCount(): number {
    if (!connectionStatus?.connected) return 0;
    return connectionStatus.proxies.filter(p => p.status === "running").length;
  }

  function getStatusSummary(): StatusSummary {
    const frpcInstance = getFrpcInstance();
    const tunnelCount = getTunnelCount();

    if (!frpcInstance) {
      return { icon: "○", label: "No frp Client", detail: "Create an instance in Services", class: "offline" };
    }
    if (!frpcInstance.running) {
      return { icon: "○", label: "frp Client Stopped", detail: "Click Start to begin", class: "stopped" };
    }
    if (connectionStatus === null) {
      return { icon: "◐", label: "Checking...", detail: "Connecting to frpc", class: "checking" };
    }
    if (!connectionStatus.running) {
      return { icon: "○", label: "Not Responding", detail: connectionStatus.error || "Cannot connect to frpc", class: "error" };
    }
    if (!connectionStatus.connected) {
      return { icon: "◐", label: "Connecting", detail: "Waiting for server connection...", class: "connecting" };
    }
    const activeCount = connectionStatus.proxies.filter(p => p.status === "running").length;
    return {
      icon: "●",
      label: "Connected",
      detail: `${activeCount}/${tunnelCount} tunnel${tunnelCount !== 1 ? 's' : ''} active`,
      class: "connected"
    };
  }

  function getProxyStatus(tunnelId: string): FrpcProxyStatus | null {
    if (!connectionStatus?.proxies) return null;
    const idPrefix = tunnelId.split('-')[0];
    const expectedName = `tunnel-${idPrefix}`;
    return connectionStatus.proxies.find(p => p.name === expectedName) || null;
  }

  function cleanup() {
    stopPolling();
  }

  return {
    get connectionStatus() { return connectionStatus; },
    loadConnectionStatus,
    startPolling,
    stopPolling,
    getConnectedTunnelCount,
    getStatusSummary,
    getProxyStatus,
    cleanup,
  };
}

export type FrpcPolling = ReturnType<typeof createFrpcPolling>;
