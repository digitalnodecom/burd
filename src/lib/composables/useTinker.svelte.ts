/**
 * Tinker state composable for PHP console
 * Handles project selection, code execution, and history
 */

import { invoke } from "@tauri-apps/api/core";

// ============================================================================
// Types
// ============================================================================

export type ProjectType = "laravel" | "wordpress" | "bedrock" | "generic";

export interface TinkerProject {
  id: string;
  path: string;
  project_type: ProjectType;
  name: string;
  instance_name: string;
}

export interface TinkerExecution {
  id: string;
  project_path: string;
  project_type: ProjectType;
  code: string;
  output: string;
  error: string | null;
  executed_at: string;
  duration_ms: number;
}

export interface TinkerPhpInfo {
  version: string | null;
  source: string | null;
  path: string | null;
  pvm_default: string | null;
  installed_versions: string[];
}

// ============================================================================
// Helpers
// ============================================================================

function getProjectTypeLabel(type: ProjectType): string {
  switch (type) {
    case "laravel":
      return "Laravel";
    case "wordpress":
      return "WordPress";
    case "bedrock":
      return "Bedrock";
    case "generic":
      return "PHP";
  }
}

function getProjectTypeColor(type: ProjectType): string {
  switch (type) {
    case "laravel":
      return "text-red-400";
    case "wordpress":
      return "text-blue-400";
    case "bedrock":
      return "text-teal-400";
    case "generic":
      return "text-purple-400";
  }
}

function formatDuration(ms: number): string {
  if (ms < 1000) {
    return `${ms}ms`;
  }
  return `${(ms / 1000).toFixed(2)}s`;
}

function formatTimeAgo(dateStr: string): string {
  const date = new Date(dateStr);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);

  if (diffSec < 60) {
    return `${diffSec}s ago`;
  } else if (diffMin < 60) {
    return `${diffMin}m ago`;
  } else if (diffHour < 24) {
    return `${diffHour}h ago`;
  } else {
    return `${diffDay}d ago`;
  }
}

// ============================================================================
// Composable
// ============================================================================

export function createTinkerState() {
  let projects = $state<TinkerProject[]>([]);
  let selectedProject = $state<TinkerProject | null>(null);
  let code = $state("");
  let output = $state("");
  let error = $state<string | null>(null);
  let executing = $state(false);
  let history = $state<TinkerExecution[]>([]);
  let phpInfo = $state<TinkerPhpInfo | null>(null);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let selectedPhpVersion = $state<string | null>(null); // null = use default

  async function loadProjects() {
    loading = true;
    loadError = null;
    try {
      projects = await invoke<TinkerProject[]>("list_tinker_projects");
      // Auto-select first project if none selected
      if (projects.length > 0 && !selectedProject) {
        selectedProject = projects[0];
      }
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
      projects = [];
    } finally {
      loading = false;
    }
  }

  async function loadPhpInfo() {
    try {
      phpInfo = await invoke<TinkerPhpInfo>("get_tinker_php_info");
    } catch (e) {
      console.error("Failed to load PHP info:", e);
      phpInfo = null;
    }
  }

  async function loadHistory() {
    try {
      history = await invoke<TinkerExecution[]>("get_tinker_history");
    } catch (e) {
      console.error("Failed to load history:", e);
      history = [];
    }
  }

  async function execute() {
    if (!selectedProject || !code.trim()) {
      return;
    }

    executing = true;
    error = null;
    output = "";

    try {
      const result = await invoke<TinkerExecution>("execute_tinker", {
        projectPath: selectedProject.path,
        projectType: selectedProject.project_type,
        code: code.trim(),
        timeoutMs: 30000,
        phpVersion: selectedPhpVersion, // null = use default
      });

      output = result.output;
      error = result.error;

      // Reload history to show the new execution
      await loadHistory();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
      output = "";
    } finally {
      executing = false;
    }
  }

  function selectProject(project: TinkerProject) {
    selectedProject = project;
  }

  function setCode(newCode: string) {
    code = newCode;
  }

  function restoreFromHistory(execution: TinkerExecution) {
    code = execution.code;
    // Try to select the same project
    const project = projects.find((p) => p.path === execution.project_path);
    if (project) {
      selectedProject = project;
    }
  }

  async function clearHistory() {
    try {
      await invoke("clear_tinker_history");
      history = [];
    } catch (e) {
      console.error("Failed to clear history:", e);
    }
  }

  async function deleteHistoryItem(id: string) {
    try {
      await invoke("delete_tinker_history_item", { id });
      history = history.filter((h) => h.id !== id);
    } catch (e) {
      console.error("Failed to delete history item:", e);
    }
  }

  function clearOutput() {
    output = "";
    error = null;
  }

  function setPhpVersion(version: string | null) {
    selectedPhpVersion = version;
  }

  return {
    // State getters
    get projects() {
      return projects;
    },
    get selectedProject() {
      return selectedProject;
    },
    get code() {
      return code;
    },
    get output() {
      return output;
    },
    get error() {
      return error;
    },
    get executing() {
      return executing;
    },
    get history() {
      return history;
    },
    get phpInfo() {
      return phpInfo;
    },
    get loading() {
      return loading;
    },
    get loadError() {
      return loadError;
    },
    get selectedPhpVersion() {
      return selectedPhpVersion;
    },
    get hasProjects() {
      return projects.length > 0;
    },
    get canExecute() {
      return selectedProject !== null && code.trim().length > 0 && !executing;
    },

    // Actions
    loadProjects,
    loadPhpInfo,
    loadHistory,
    execute,
    selectProject,
    setCode,
    setPhpVersion,
    restoreFromHistory,
    clearHistory,
    deleteHistoryItem,
    clearOutput,

    // Helpers
    getProjectTypeLabel,
    getProjectTypeColor,
    formatDuration,
    formatTimeAgo,
  };
}

export type TinkerState = ReturnType<typeof createTinkerState>;
