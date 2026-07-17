/**
 * App auto-update composable.
 *
 * Wraps the Tauri updater plugin: checks GitHub Releases for a newer signed
 * build, downloads + installs it on demand, and relaunches. All update
 * artifacts are minisign-verified by the plugin before install.
 */

import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export function createUpdater() {
  let update = $state<Update | null>(null);
  let checking = $state(false);
  let downloading = $state(false);
  let downloaded = $state(0);
  let total = $state(0);
  let error = $state<string | null>(null);
  // Set once the user dismisses an available update, so we stop nagging.
  let dismissed = $state(false);

  const available = $derived(update !== null && !dismissed);
  const progress = $derived(total > 0 ? Math.round((downloaded / total) * 100) : 0);

  /** Check for a newer release. Silent on failure (e.g. offline). */
  async function checkForUpdate(): Promise<void> {
    if (checking || downloading) return;
    checking = true;
    error = null;
    try {
      const result = await check();
      // `check()` returns null when already up to date.
      update = result ?? null;
      if (result) dismissed = false;
    } catch (e) {
      // Network / manifest errors shouldn't surface as scary UI on startup.
      error = String(e);
    } finally {
      checking = false;
    }
  }

  /** Download + install the pending update, then relaunch into it. */
  async function installAndRestart(): Promise<void> {
    if (!update || downloading) return;
    downloading = true;
    error = null;
    downloaded = 0;
    total = 0;
    try {
      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            total = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded += event.data.chunkLength;
            break;
        }
      });
      await relaunch();
    } catch (e) {
      error = String(e);
      downloading = false;
    }
  }

  function dismiss() {
    dismissed = true;
  }

  // Poll for updates on an interval (default 30 min) in addition to the
  // startup check, so long-running windows still notice new releases.
  let timer: ReturnType<typeof setInterval> | null = null;
  function startAutoCheck(intervalMs = 30 * 60 * 1000) {
    if (timer) return;
    timer = setInterval(() => {
      void checkForUpdate();
    }, intervalMs);
  }
  function stopAutoCheck() {
    if (timer) {
      clearInterval(timer);
      timer = null;
    }
  }

  return {
    get available() {
      return available;
    },
    get version() {
      return update?.version ?? null;
    },
    get notes() {
      return update?.body ?? null;
    },
    get checking() {
      return checking;
    },
    get downloading() {
      return downloading;
    },
    get progress() {
      return progress;
    },
    get error() {
      return error;
    },
    checkForUpdate,
    installAndRestart,
    dismiss,
    startAutoCheck,
    stopAutoCheck,
  };
}
