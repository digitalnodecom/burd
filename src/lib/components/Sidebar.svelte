<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import type { Component } from 'svelte';
  import {
    Settings, Globe, Package, Download, Layers, SquareTerminal,
    ScrollText, Mail, Zap, FolderTree, Monitor, Sun, Moon,
  } from '@lucide/svelte';

  let appVersion = $state('');

  type Section = {
    id: string;
    label: string;
    icon?: Component; // Lucide icon component
    brand?: 'php'; // brand logos we keep as-is (not Lucide)
  };

  type Theme = 'system' | 'light' | 'dark';

  interface Updater {
    readonly available: boolean;
    readonly version: string | null;
    readonly downloading: boolean;
    readonly progress: number;
    installAndRestart: () => Promise<void>;
  }

  interface Props {
    activeSection: string;
    onNavigate: (id: string) => void;
    mailpitExists?: boolean;
    unreadMailCount?: number;
    frpcDownloaded?: boolean;
    parkEnabled?: boolean;
    updater?: Updater;
  }

  let { activeSection = $bindable(), onNavigate, mailpitExists = false, unreadMailCount = 0, frpcDownloaded = false, parkEnabled = false, updater }: Props = $props();

  let theme = $state<Theme>('system');

  function getSystemTheme(): 'light' | 'dark' {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  function applyTheme(t: Theme) {
    const effectiveTheme = t === 'system' ? getSystemTheme() : t;
    document.documentElement.setAttribute('data-theme', effectiveTheme);
  }

  function cycleTheme() {
    const order: Theme[] = ['system', 'light', 'dark'];
    const currentIndex = order.indexOf(theme);
    theme = order[(currentIndex + 1) % order.length];
    localStorage.setItem('burd-theme', theme);
    applyTheme(theme);
  }

  onMount(() => {
    const stored = localStorage.getItem('burd-theme') as Theme | null;
    if (stored && ['system', 'light', 'dark'].includes(stored)) {
      theme = stored;
    }
    applyTheme(theme);

    // Get app version
    getVersion().then((v) => {
      appVersion = v;
    }).catch(() => {
      appVersion = '';
    });

    // Listen for system theme changes
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleChange = () => {
      if (theme === 'system') {
        applyTheme('system');
      }
    };
    mediaQuery.addEventListener('change', handleChange);
    return () => mediaQuery.removeEventListener('change', handleChange);
  });

  const baseSections: Section[] = [
    { id: 'general', label: 'General', icon: Settings },
    { id: 'domains', label: 'Domains', icon: Globe },
    { id: 'instances', label: 'Instances', icon: Package },
    { id: 'services', label: 'Services', icon: Download },
    { id: 'node', label: 'Node', icon: Layers },
    { id: 'php', label: 'PHP', brand: 'php' },
    { id: 'tinker', label: 'Tinker', icon: SquareTerminal },
    { id: 'logs', label: 'Logs', icon: ScrollText },
  ];

  const mailSection: Section = { id: 'mail', label: 'Mail', icon: Mail };
  const tunnelsSection: Section = { id: 'tunnels', label: 'Tunnels', icon: Zap };
  const parksSection: Section = { id: 'parks', label: 'Parks', icon: FolderTree };

  // Build sections array with conditional items
  const sections = $derived.by(() => {
    const result = [...baseSections];
    // Insert mail after instances (index 3) if mailpit exists
    if (mailpitExists) {
      result.splice(3, 0, mailSection);
    }
    // Insert tunnels after services (index 4, or 5 if mail was added) - only when frpc is downloaded
    if (frpcDownloaded) {
      const tunnelsIndex = mailpitExists ? 5 : 4;
      result.splice(tunnelsIndex, 0, tunnelsSection);
    }
    // Add parks at the end (after domains, before instances) when enabled
    if (parkEnabled) {
      // Insert parks after domains (index 1)
      result.splice(2, 0, parksSection);
    }
    return result;
  });

  function handleClick(id: string) {
    activeSection = id;
    onNavigate(id);
  }
</script>

<nav class="sidebar">
  <div class="sidebar-header">
    <div class="header-row">
      <h1>Burd</h1>
      <button class="theme-toggle" onclick={cycleTheme} title={theme === 'system' ? 'System theme' : theme === 'light' ? 'Light mode' : 'Dark mode'}>
        {#if theme === 'system'}
          <Monitor size={16} strokeWidth={2} />
        {:else if theme === 'light'}
          <Sun size={16} strokeWidth={2} />
        {:else}
          <Moon size={16} strokeWidth={2} />
        {/if}
      </button>
    </div>
    <span class="subtitle">Service Manager</span>
  </div>

  <div class="sidebar-nav">
    {#each sections as section}
      <button
        class="nav-item"
        class:active={activeSection === section.id}
        onclick={() => handleClick(section.id)}
      >
        {#if section.brand === 'php'}
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
            <path fill-rule="evenodd" d="M7.01 10.207h-.944l-.515 2.648h.838c.556 0 .97-.105 1.242-.314.272-.21.455-.559.55-1.049.092-.47.05-.802-.124-.995-.175-.193-.523-.29-1.047-.29zM12 5.688C5.373 5.688 0 8.514 0 12s5.373 6.313 12 6.313S24 15.486 24 12c0-3.486-5.373-6.312-12-6.312zm-3.26 7.451c-.261.25-.575.438-.917.551-.336.108-.765.164-1.285.164H5.357l-.327 1.681H3.652l1.23-6.326h2.65c.797 0 1.378.209 1.744.628.366.418.476 1.002.33 1.752a2.836 2.836 0 0 1-.305.847c-.143.255-.33.49-.561.703zm4.024.715l.543-2.799c.063-.318.039-.536-.068-.651-.107-.116-.336-.174-.687-.174H11.46l-.704 3.625H9.388l1.23-6.327h1.367l-.327 1.682h1.218c.767 0 1.295.134 1.586.401s.378.7.263 1.299l-.572 2.944h-1.389zm7.597-2.265a2.782 2.782 0 0 1-.305.847c-.143.255-.33.49-.561.703a2.44 2.44 0 0 1-.917.551c-.336.108-.765.164-1.286.164h-1.18l-.327 1.682h-1.378l1.23-6.326h2.649c.797 0 1.378.209 1.744.628.366.417.477 1.001.331 1.751zM17.766 10.207h-.943l-.516 2.648h.838c.557 0 .971-.105 1.242-.314.272-.21.455-.559.551-1.049.092-.47.049-.802-.125-.995s-.524-.29-1.047-.29z"></path>
          </svg>
        {:else if section.icon}
          {@const Icon = section.icon}
          <Icon size={20} strokeWidth={2} />
        {/if}
        <span>{section.label}</span>
        {#if section.id === 'mail' && unreadMailCount > 0}
          <span class="badge">{unreadMailCount > 99 ? '99+' : unreadMailCount}</span>
        {/if}
      </button>
    {/each}
  </div>

  {#if appVersion}
    <div class="sidebar-footer">
      <span class="version">v{appVersion}</span>
      {#if updater?.available}
        <button
          class="update-badge"
          class:downloading={updater.downloading}
          onclick={() => updater.installAndRestart()}
          disabled={updater.downloading}
          title={updater.downloading
            ? `Updating to ${updater.version}… ${updater.progress}%`
            : `Update to ${updater.version} available — click to install & restart`}
          aria-label="Install update"
        >
          {#if updater.downloading}
            <span class="update-spinner"></span>
          {:else}
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 19V5M5 12l7-7 7 7" />
            </svg>
          {/if}
        </button>
      {/if}
    </div>
  {/if}
</nav>

<style>
  .sidebar {
    width: 220px;
    min-width: 220px;
    background: #f5f5f7;
    border-right: 1px solid #e0e0e0;
    display: flex;
    flex-direction: column;
    height: 100vh;
    position: sticky;
    top: 0;
  }

  .sidebar-header {
    padding: 20px;
    border-bottom: 1px solid #e0e0e0;
  }

  .header-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .theme-toggle {
    background: transparent;
    border: none;
    padding: 6px;
    border-radius: 6px;
    cursor: pointer;
    color: #86868b;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .theme-toggle:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  .sidebar-header h1 {
    margin: 0;
    font-size: 24px;
    font-weight: 700;
    background: linear-gradient(135deg, #ff6b6b 0%, #ee5a24 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .sidebar-header .subtitle {
    font-size: 12px;
    color: #86868b;
    margin-top: 2px;
    display: block;
  }

  .sidebar-nav {
    padding: 12px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
  }

  .sidebar-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 20px;
    border-top: 1px solid rgba(0, 0, 0, 0.1);
  }

  .version {
    font-size: 11px;
    color: #86868b;
  }

  .update-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: #34c759;
    color: white;
    cursor: pointer;
    flex: none;
    transition: transform 0.1s ease, opacity 0.1s ease;
  }
  .update-badge:hover {
    transform: scale(1.12);
  }
  .update-badge:disabled {
    cursor: default;
    background: #86868b;
  }
  .update-spinner {
    width: 10px;
    height: 10px;
    border: 2px solid rgba(255, 255, 255, 0.4);
    border-top-color: white;
    border-radius: 50%;
    animation: update-spin 0.7s linear infinite;
  }
  @keyframes update-spin {
    to {
      transform: rotate(360deg);
    }
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border: none;
    background: transparent;
    border-radius: 8px;
    cursor: pointer;
    font-size: 14px;
    font-weight: 500;
    color: #1d1d1f;
    text-align: left;
    transition: all 0.15s ease;
  }

  .nav-item:hover {
    background: rgba(0, 0, 0, 0.05);
  }

  .nav-item.active {
    background: #007aff;
    color: white;
  }

  .nav-item.active svg {
    stroke: white;
  }

  .nav-item svg {
    flex-shrink: 0;
    stroke: #636366;
  }

  .nav-item:hover svg {
    stroke: #1d1d1f;
  }

  .nav-item.active:hover {
    background: #0066d6;
  }

  .badge {
    margin-left: auto;
    background: linear-gradient(135deg, #ff6b6b, #ee5a24);
    color: white;
    font-size: 11px;
    font-weight: 600;
    padding: 2px 6px;
    border-radius: 10px;
    min-width: 18px;
    text-align: center;
  }

  .nav-item.active .badge {
    background: rgba(255, 255, 255, 0.25);
  }

  /* Dark mode - media query (for system theme) */
  @media (prefers-color-scheme: dark) {
    .sidebar {
      background: #1c1c1e;
      border-right-color: #38383a;
    }

    .sidebar-header {
      border-bottom-color: #38383a;
    }

    .sidebar-footer {
      border-top-color: #38383a;
    }

    .version {
      color: #98989d;
    }

    .theme-toggle {
      color: #98989d;
    }

    .theme-toggle:hover {
      background: rgba(255, 255, 255, 0.1);
      color: #f5f5f7;
    }

    .nav-item {
      color: #f5f5f7;
    }

    .nav-item:hover {
      background: rgba(255, 255, 255, 0.1);
    }

    .nav-item svg {
      stroke: #98989d;
    }

    .nav-item:hover svg {
      stroke: #f5f5f7;
    }

    .nav-item.active {
      background: #0a84ff;
    }

    .nav-item.active:hover {
      background: #0077ed;
    }
  }

  /* Explicit dark mode via data-theme attribute */
  :global(:root[data-theme="dark"]) .sidebar {
    background: #1c1c1e;
    border-right-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .sidebar-header {
    border-bottom-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .sidebar-footer {
    border-top-color: #38383a;
  }

  :global(:root[data-theme="dark"]) .version {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .theme-toggle {
    color: #98989d;
  }

  :global(:root[data-theme="dark"]) .theme-toggle:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .nav-item {
    color: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .nav-item:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  :global(:root[data-theme="dark"]) .nav-item svg {
    stroke: #98989d;
  }

  :global(:root[data-theme="dark"]) .nav-item:hover svg {
    stroke: #f5f5f7;
  }

  :global(:root[data-theme="dark"]) .nav-item.active {
    background: #0a84ff;
  }

  :global(:root[data-theme="dark"]) .nav-item.active:hover {
    background: #0077ed;
  }

  /* Explicit light mode via data-theme attribute (overrides system dark) */
  :global(:root[data-theme="light"]) .sidebar {
    background: #f5f5f7;
    border-right-color: #e0e0e0;
  }

  :global(:root[data-theme="light"]) .sidebar-header {
    border-bottom-color: #e0e0e0;
  }

  :global(:root[data-theme="light"]) .sidebar-footer {
    border-top-color: #e0e0e0;
  }

  :global(:root[data-theme="light"]) .theme-toggle {
    color: #86868b;
  }

  :global(:root[data-theme="light"]) .theme-toggle:hover {
    background: rgba(0, 0, 0, 0.05);
    color: #1d1d1f;
  }

  :global(:root[data-theme="light"]) .nav-item {
    color: #1d1d1f;
  }

  :global(:root[data-theme="light"]) .nav-item:hover {
    background: rgba(0, 0, 0, 0.05);
  }

  :global(:root[data-theme="light"]) .nav-item svg {
    stroke: #636366;
  }

  :global(:root[data-theme="light"]) .nav-item:hover svg {
    stroke: #1d1d1f;
  }

  :global(:root[data-theme="light"]) .nav-item.active {
    background: #007aff;
    color: white;
  }

  :global(:root[data-theme="light"]) .nav-item.active svg {
    stroke: white;
  }

  :global(:root[data-theme="light"]) .nav-item.active:hover {
    background: #0066d6;
  }

  /* Light mode badge overrides */
  :global(:root[data-theme="light"]) .badge {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24) !important;
    color: white !important;
  }

  :global(:root[data-theme="light"]) .nav-item.active .badge {
    background: rgba(255, 255, 255, 0.3) !important;
    color: white !important;
  }

  /* Dark mode badge overrides */
  :global(:root[data-theme="dark"]) .badge {
    background: linear-gradient(135deg, #ff6b6b, #ee5a24) !important;
    color: white !important;
  }

  :global(:root[data-theme="dark"]) .nav-item.active .badge {
    background: rgba(255, 255, 255, 0.35) !important;
    color: white !important;
  }

  /* Media query badge overrides for system theme */
  @media (prefers-color-scheme: dark) {
    .nav-item.active .badge {
      background: rgba(255, 255, 255, 0.35);
    }
  }
</style>
