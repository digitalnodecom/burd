<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';

  let appVersion = $state('');

  type Section = {
    id: string;
    label: string;
    icon: string;
    filled?: boolean; // true for fill-based icons (like PHP logo)
  };

  type Theme = 'system' | 'light' | 'dark';

  interface Props {
    activeSection: string;
    onNavigate: (id: string) => void;
    mailpitExists?: boolean;
    unreadMailCount?: number;
    frpcInstanceExists?: boolean;
    parkEnabled?: boolean;
  }

  let { activeSection = $bindable(), onNavigate, mailpitExists = false, unreadMailCount = 0, frpcInstanceExists = false, parkEnabled = false }: Props = $props();

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

  onMount(async () => {
    const stored = localStorage.getItem('burd-theme') as Theme | null;
    if (stored && ['system', 'light', 'dark'].includes(stored)) {
      theme = stored;
    }
    applyTheme(theme);

    // Get app version
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = '';
    }

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
    { id: 'general', label: 'General', icon: 'M12 15a3 3 0 100-6 3 3 0 000 6z M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z' },
    { id: 'domains', label: 'Domains', icon: 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 17.93c-3.95-.49-7-3.85-7-7.93 0-.62.08-1.21.21-1.79L9 15v1c0 1.1.9 2 2 2v1.93zm6.9-2.54c-.26-.81-1-1.39-1.9-1.39h-1v-3c0-.55-.45-1-1-1H8v-2h2c.55 0 1-.45 1-1V7h2c1.1 0 2-.9 2-2v-.41c2.93 1.19 5 4.06 5 7.41 0 2.08-.8 3.97-2.1 5.39z' },
    { id: 'instances', label: 'Instances', icon: 'M20 7l-8-4-8 4m16 0l-8 4m8-4v10l-8 4m0-10L4 7m8 4v10M4 7v10l8 4' },
    { id: 'services', label: 'Services', icon: 'M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4' },
    { id: 'node', label: 'Node', icon: 'M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5' },
    { id: 'php', label: 'PHP', filled: true, icon: 'M7.01 10.207h-.944l-.515 2.648h.838c.556 0 .97-.105 1.242-.314.272-.21.455-.559.55-1.049.092-.47.05-.802-.124-.995-.175-.193-.523-.29-1.047-.29zM12 5.688C5.373 5.688 0 8.514 0 12s5.373 6.313 12 6.313S24 15.486 24 12c0-3.486-5.373-6.312-12-6.312zm-3.26 7.451c-.261.25-.575.438-.917.551-.336.108-.765.164-1.285.164H5.357l-.327 1.681H3.652l1.23-6.326h2.65c.797 0 1.378.209 1.744.628.366.418.476 1.002.33 1.752a2.836 2.836 0 0 1-.305.847c-.143.255-.33.49-.561.703zm4.024.715l.543-2.799c.063-.318.039-.536-.068-.651-.107-.116-.336-.174-.687-.174H11.46l-.704 3.625H9.388l1.23-6.327h1.367l-.327 1.682h1.218c.767 0 1.295.134 1.586.401s.378.7.263 1.299l-.572 2.944h-1.389zm7.597-2.265a2.782 2.782 0 0 1-.305.847c-.143.255-.33.49-.561.703a2.44 2.44 0 0 1-.917.551c-.336.108-.765.164-1.286.164h-1.18l-.327 1.682h-1.378l1.23-6.326h2.649c.797 0 1.378.209 1.744.628.366.417.477 1.001.331 1.751zM17.766 10.207h-.943l-.516 2.648h.838c.557 0 .971-.105 1.242-.314.272-.21.455-.559.551-1.049.092-.47.049-.802-.125-.995s-.524-.29-1.047-.29z' },
    { id: 'tinker', label: 'Tinker', icon: 'M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z' },
    { id: 'logs', label: 'Logs', icon: 'M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2m-3 7h3m-3 4h3m-6-4h.01M9 16h.01' },
  ];

  // Mail section (envelope icon) - only shown when Mailpit exists
  const mailSection: Section = {
    id: 'mail',
    label: 'Mail',
    icon: 'M3 8l7.89 5.26a2 2 0 002.22 0L21 8M5 19h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z'
  };

  // Tunnels section (lightning icon) - only shown when frpc is downloaded
  const tunnelsSection: Section = {
    id: 'tunnels',
    label: 'Tunnels',
    icon: 'M13 10V3L4 14h7v7l9-11h-7z'
  };

  // Parks section (folder icon) - only shown when FrankenPHP Park is enabled
  const parksSection: Section = {
    id: 'parks',
    label: 'Parks',
    icon: 'M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z'
  };

  // Build sections array with conditional items
  const sections = $derived.by(() => {
    const result = [...baseSections];
    // Insert mail after instances (index 3) if mailpit exists
    if (mailpitExists) {
      result.splice(3, 0, mailSection);
    }
    // Insert tunnels after services (index 4, or 5 if mail was added) - only when instance exists
    if (frpcInstanceExists) {
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
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect>
            <line x1="8" y1="21" x2="16" y2="21"></line>
            <line x1="12" y1="17" x2="12" y2="21"></line>
          </svg>
        {:else if theme === 'light'}
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="5"></circle>
            <line x1="12" y1="1" x2="12" y2="3"></line>
            <line x1="12" y1="21" x2="12" y2="23"></line>
            <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line>
            <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line>
            <line x1="1" y1="12" x2="3" y2="12"></line>
            <line x1="21" y1="12" x2="23" y2="12"></line>
            <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line>
            <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path>
          </svg>
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
        {#if section.filled}
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor">
            <path fill-rule="evenodd" d={section.icon}></path>
          </svg>
        {:else}
          <svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d={section.icon}></path>
          </svg>
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
    padding: 12px 20px;
    border-top: 1px solid rgba(0, 0, 0, 0.1);
  }

  .version {
    font-size: 11px;
    color: #86868b;
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
