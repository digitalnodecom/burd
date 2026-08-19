// Dev-only Tauri shim for iterating on the UI in a plain browser.
//
// Tauri injects `window.__TAURI_INTERNALS__` in the real app; a browser has no
// such thing, so `invoke()` throws "Cannot read properties of undefined". This
// installs a mock that returns representative sample data, so the whole UI
// renders and can be visually tested without building the Rust app.
//
// Guarded by `import.meta.env.DEV` so it is stripped from production builds, and
// it never overrides a real Tauri runtime.
if (import.meta.env.DEV && typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) {
  // Representative fixtures for the commands the dashboard calls on load.
  const instances = [
    inst("git", "gitea", "1.22.3", true, 3099, "git", 2_940_000),
    inst("postgres", "postgresql", "17.10", true, 5432, "", 4_509_715_660),
    inst("bgdb", "mariadb", "12.3.2", true, 3332, "", 21_139_998_720),
    inst("getapp", "frankenphp", "8.4-1.12.4", false, 8005, "app.get", 535),
    inst("mail", "mailpit", "1.30.4", true, 8025, "mail", 0),
    inst("cache", "redis", "8.8.0", false, 6380, "", 128_000),
  ];

  function inst(
    name: string,
    service_type: string,
    version: string,
    running: boolean,
    port: number,
    subdomain: string,
    size: number,
  ) {
    const id = crypto.randomUUID();
    (inst as any)._sizes = (inst as any)._sizes || {};
    (inst as any)._sizes[id] = size;
    return {
      id,
      name,
      port,
      service_type,
      version,
      running,
      pid: running ? 1000 + port : null,
      healthy: running ? true : null,
      has_config: true,
      domain: subdomain ? `${subdomain}.test` : "",
      domain_enabled: !!subdomain,
      process_manager: "binary",
      stack_id: null,
      mapped_domains: subdomain ? [`${subdomain}.test`] : [],
      auto_start: running && service_type !== "frankenphp",
    };
  }

  const handlers: Record<string, (args: any) => unknown> = {
    list_instances: () => instances,
    get_instance_disk_usage: (a) => (inst as any)._sizes?.[a?.instanceId] ?? 0,
    list_stacks: () => [],
    get_network_status: () => ({ dns_running: true, tld: "test", resolver_installed: true }),
    get_proxy_status: () => ({
      daemon_installed: true,
      daemon_running: true,
      daemon_pid: 99176,
      caddy_installed: true,
      proxy_healthy: true,
    }),
    get_ca_trust_status: () => ({ ca_exists: true, is_trusted: true, ca_path: "", cert_name: null, cert_expiry: null }),
    get_cli_status: () => ({ installed: true, path: "/usr/local/bin/burd" }),
    get_helper_status: () => ({ installed: true }),
    is_park_enabled: () => false,
    get_available_services: () => [
      svc("frankenphp", "PHP", 8000),
      svc("postgresql", "PostgreSQL", 5432),
      svc("mariadb", "MariaDB", 3306),
      svc("redis", "Redis", 6379),
      svc("mailpit", "Mailpit", 8025),
    ],
    get_all_binary_statuses: () => [],
    get_installed_versions: (a) => versionsFor(a?.serviceType),
    get_settings: () => ({ tld: "test", auto_start: true }),
    list_domains: () => [],
    // Per-instance detail commands (settings modal, env, info panels).
    get_instance_config: (a) => {
      const i = instances.find((x) => x.id === a?.id);
      return {
        id: a?.id,
        name: i?.name ?? "instance",
        service_type: i?.service_type ?? "frankenphp",
        config: {},
      };
    },
    get_instance_info: (a) => {
      const i = instances.find((x) => x.id === a?.id);
      return {
        id: a?.id,
        name: i?.name ?? "instance",
        service_type: i?.service_type ?? "frankenphp",
        version: i?.version ?? "",
        port: i?.port ?? 0,
        running: i?.running ?? false,
        pid: i?.pid ?? null,
        categories: [],
      };
    },
    get_instance_env: () => ({ DATABASE_URL: "postgres://postgres@127.0.0.1:5432/app" }),
    generate_env_for_service: () => "",
    check_instance_health: () => true,
    list_instance_databases: () => ["app", "app_test"],
    list_instance_database_details: () => [
      { name: "app", size: 21_139_998_720, tables: null },
      { name: "app_test", size: 4_096_000, tables: null },
    ],
  };

  function versionsFor(serviceType?: string): string[] {
    const map: Record<string, string[]> = {
      frankenphp: ["8.4-1.12.4", "8.3-1.12.4", "8.5-1.12.4"],
      postgresql: ["17.10"],
      mariadb: ["12.3.2"],
      redis: ["8.8.0"],
      mailpit: ["1.30.4"],
      gitea: ["1.22.3"],
    };
    return map[serviceType ?? ""] ?? ["1.0.0"];
  }

  function svc(id: string, display_name: string, default_port: number) {
    return {
      id,
      display_name,
      default_port,
      config_fields: [],
      available: true,
      is_homebrew: false,
      process_manager: "binary",
    };
  }

  (window as any).__TAURI_INTERNALS__ = {
    transformCallback: (cb: unknown) => {
      const id = Math.floor(Math.random() * 1e9);
      (window as any)[`_${id}`] = cb;
      return id;
    },
    async invoke(cmd: string, args: any) {
      // Event + plugin calls (updater/dialog/process/event): no-op-ish.
      if (cmd.startsWith("plugin:event|")) return 0;
      if (cmd.startsWith("plugin:")) return null;
      if (cmd in handlers) return handlers[cmd](args);
      // eslint-disable-next-line no-console
      console.warn("[tauri-mock] unhandled command:", cmd, args);
      return null;
    },
  };
  // eslint-disable-next-line no-console
  console.info("[tauri-mock] installed — browser UI running against sample data");

  // Dev helper: `#_shot=<selector>` clicks a matching element after the UI
  // settles, so modals/sections can be captured headlessly for review.
  const shot = new URLSearchParams(location.hash.replace(/^#/, "")).get("_shot");
  if (shot) {
    const steps = decodeURIComponent(shot).split("|");
    const run = (i: number, attempt = 0) => {
      if (i >= steps.length) return;
      const el = document.querySelector<HTMLElement>(steps[i]);
      if (el) {
        el.click();
        setTimeout(() => run(i + 1), 500);
      } else if (attempt < 30) {
        setTimeout(() => run(i, attempt + 1), 200);
      }
    };
    setTimeout(() => run(0), 500);
  }
}
