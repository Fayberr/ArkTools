import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  currentMonitor,
  getCurrentWindow,
  LogicalSize,
} from "@tauri-apps/api/window";
import { lazy, useEffect, useRef, useState } from "react";
import { applyAccentTheme } from "./accentTheme";
import { I18nProvider } from "./i18n";
import {
  sanitizeSettings,
  type Settings,
} from "./settingsSchema";
import {
  APP_VERSION,
  DEFAULT_SETTINGS,
  type AppInfo,
  type ClickerStatus,
  clearSavedSettings,
  loadSettings,
  saveSettings,
} from "./store";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const MacrosPanel = lazy(() => import("./components/panels/MacrosPanel"));
const SettingsPanel = lazy(() => import("./components/panels/SettingsPanel"));
const TitleBar = lazy(() => import("./components/TitleBar"));

export type Tab = "macros" | "settings";

const MAX_DROPDOWN_OVERFLOW_BOTTOM = 220;

function getPanelSize(tab: Tab) {
  if (tab === "macros") {
    return { width: 560, height: 420 };
  }
  return { width: 560, height: 600 };
}

const textScale = await invoke<number>("get_text_scale_factor");
await invoke("set_webview_zoom", { factor: 1.0 / textScale });

async function getClampedPanelSize(
  size: { width: number; height: number },
  textScale: number,
) {
  const monitor = await currentMonitor();
  if (!monitor) return size;

  const scale = monitor.scaleFactor || 1;
  const workAreaWidth = Math.floor(monitor.workArea.size.width / scale);
  const workAreaHeight = Math.floor(monitor.workArea.size.height / scale);
  const horizontalMargin = 24;
  const verticalMargin = 24;

  return {
    width: Math.min(
      Math.ceil(size.width * textScale),
      Math.max(360, workAreaWidth - horizontalMargin),
    ),
    height: Math.min(
      Math.ceil(size.height * textScale),
      Math.max(220, workAreaHeight - verticalMargin),
    ),
  };
}

const DEFAULT_STATUS: ClickerStatus = {
  running: false,
  clickCount: 0,
  lastError: null,
  stopReason: null,
  activeSequenceIndex: null,
  activeSequenceTick: 0,
};

const DEFAULT_APP_INFO: AppInfo = {
  version: APP_VERSION,
  updateStatus: "Update checks are disabled",
  screenshotProtectionSupported: false,
};

async function syncSettingsToBackend(settings: Settings) {
  await invoke("update_settings", {
    settings,
  });
}

function wait(ms: number) {
  return new Promise((resolve) => window.setTimeout(resolve, ms));
}

export default function App() {
  const [tab, setTab] = useState<Tab>("macros");
  const [settings, setSettings] = useState<Settings>(DEFAULT_SETTINGS);
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [status, setStatus] = useState<ClickerStatus>(DEFAULT_STATUS);
  const [appInfo, setAppInfo] = useState<AppInfo>(DEFAULT_APP_INFO);
  const [dropdownOverflowBottom, setDropdownOverflowBottom] = useState(0);

  const [updateStatus, setUpdateStatus] = useState<"idle" | "checking" | "up-to-date" | "available" | "downloading" | "error">("idle");
  const [availableUpdate, setAvailableUpdate] = useState<Update | null>(null);
  const [updateError, setUpdateError] = useState<string | null>(null);

  const launchWindowPlacementDone = useRef(false);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleCheckForUpdate = async (isManual = false) => {
    setUpdateStatus("checking");
    setUpdateError(null);
    try {
      const update = await check();
      if (update) {
        setAvailableUpdate(update);
        setUpdateStatus("available");
        
        if (!isManual && settings.autoUpdate) {
          setUpdateStatus("downloading");
          await update.downloadAndInstall();
          await relaunch();
        }
      } else {
        setAvailableUpdate(null);
        setUpdateStatus("up-to-date");
      }
    } catch (err) {
      console.error("Update check failed:", err);
      if (isManual) {
        setUpdateError(err instanceof Error ? err.message : String(err));
        setUpdateStatus("error");
      } else {
        setUpdateStatus("idle");
      }
    }
  };

  const handleApplyUpdate = async () => {
    if (!availableUpdate) return;
    setUpdateStatus("downloading");
    setUpdateError(null);
    try {
      await availableUpdate.downloadAndInstall();
      await relaunch();
    } catch (err) {
      console.error("Manual update install failed:", err);
      setUpdateError(err instanceof Error ? err.message : String(err));
      setUpdateStatus("error");
    }
  };

  const scheduleSave = (nextSettings: Settings) => {
    if (saveTimerRef.current) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(() => {
      saveSettings(nextSettings).catch((err) => {
        console.error("Failed to save settings:", err);
      });
    }, 100);
  };

  const updateSettings = (patch: Partial<Settings>) => {
    const nextSettings = sanitizeSettings(
      { ...settings, ...patch },
      APP_VERSION,
    );
    setSettings(nextSettings);

    if (!settingsLoaded) return;

    syncSettingsToBackend(nextSettings).catch((err) => {
      console.error("Failed to sync settings to backend:", err);
    });
    scheduleSave(nextSettings);
  };

  const applyStartupWindowPlacement = async () => {
    await getCurrentWindow().center();
  };

  const handleWindowClose = async () => {
    if (settings.minimizeToTray) {
      await getCurrentWindow().hide();
    } else {
      await invoke("quit_app");
    }
  };

  const handleToggleAlwaysOnTop = async () => {
    const nextValue = !settings.alwaysOnTop;

    try {
      await getCurrentWindow().setAlwaysOnTop(nextValue);
      updateSettings({
        alwaysOnTop: nextValue,
      });
    } catch (err) {
      console.error("Failed to set always on top:", err);
    }
  };

  useEffect(() => {
    let mounted = true;

    void Promise.all([
      loadSettings(),
      invoke<AppInfo>("get_app_info"),
      invoke<ClickerStatus>("get_status"),
    ])
      .then(async ([loadedSettings, loadedAppInfo, loadedStatus]) => {
        if (!mounted) return;

        setTab(loadedSettings.lastPanel);
        setSettings(loadedSettings);
        setAppInfo(loadedAppInfo);
        setStatus(loadedStatus);
        setSettingsLoaded(true);

        await syncSettingsToBackend(loadedSettings);

        // Check for updates silently in the background
        void (async () => {
          try {
            const update = await check();
            if (update) {
              console.log(`Update available: ${update.version}`);
              setAvailableUpdate(update);
              setUpdateStatus("available");
              if (loadedSettings.autoUpdate) {
                setUpdateStatus("downloading");
                await update.downloadAndInstall();
                await relaunch();
              }
            } else {
              setUpdateStatus("up-to-date");
            }
          } catch (err) {
            console.warn("Failed to check for updates (local/remote server down?):", err);
          }
        })();

        try {
          await getCurrentWindow().setAlwaysOnTop(loadedSettings.alwaysOnTop);
        } catch (err) {
          console.error("Failed to restore always on top:", err);
        }
      })
      .catch((err) => {
        console.error("Failed to boot app:", err);
        if (!mounted) return;
        setSettingsLoaded(true);
      });

    return () => {
      mounted = false;
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    let cleanup: (() => void) | undefined;

    listen<ClickerStatus>("clicker-status", (event) => {
      setStatus(event.payload);
    })
      .then((unlisten) => {
        cleanup = unlisten;
      })
      .catch((err) => {
        console.error("Failed to listen for status:", err);
      });

    return () => {
      cleanup?.();
    };
  }, []);

  useEffect(() => {
    const handleDropdownOverflow = (event: Event) => {
      const { active, bottom = 0 } = (
        event as CustomEvent<{ active: boolean; bottom?: number }>
      ).detail;
      const nextOverflow = active
        ? Math.min(Math.max(0, bottom), MAX_DROPDOWN_OVERFLOW_BOTTOM)
        : 0;

      setDropdownOverflowBottom(nextOverflow);
    };

    window.addEventListener("blur-dropdown-overflow", handleDropdownOverflow);

    return () => {
      window.removeEventListener(
        "blur-dropdown-overflow",
        handleDropdownOverflow,
      );
    };
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const textScale = await invoke<number>("get_text_scale_factor");
        document.documentElement.style.fontSize = `${16 * textScale}px`;

        const preferredSize = getPanelSize(tab);
        const { width, height } = await getClampedPanelSize(
          preferredSize,
          textScale,
        );
        const windowHeight = height + dropdownOverflowBottom;

        const appWindow = getCurrentWindow();

        await appWindow.setSize(new LogicalSize(width, windowHeight));

        if (!launchWindowPlacementDone.current) {
          await wait(30);
          await applyStartupWindowPlacement();
          
          // Workaround for Tauri v2 transparent window border/corner rendering bug on Windows:
          // Slightly resize and restore to trigger compositor refresh and ensure rounded corners
          await appWindow.setSize(new LogicalSize(width + 1, windowHeight + 1));
          await wait(15);
          await appWindow.setSize(new LogicalSize(width, windowHeight));
          
          launchWindowPlacementDone.current = true;
        }
      } catch (err) {
        console.error("Failed to size window:", err);
      }
    })();
  }, [settingsLoaded, tab, dropdownOverflowBottom]);

  useEffect(() => {
    const theme = settings.theme ?? "dark";
    document.documentElement.dataset.theme = theme;
    applyAccentTheme(settings.accentColor, theme);
  }, [settings.accentColor, settings.theme]);

  useEffect(() => {
    document.documentElement.lang = settings.language;
  }, [settings.language]);

  const handleTabChange = (nextTab: Tab) => {
    setTab(nextTab);

    if (nextTab === "settings") return;
    if (settings.lastPanel === nextTab) return;

    updateSettings({
      lastPanel: nextTab,
    });
  };

  const handleResetSettings = async () => {
    try {
      await invoke("reset_settings");
      await clearSavedSettings();
      await invoke("set_autostart_enabled", { enabled: false }).catch(() => { });
      await getCurrentWindow().setAlwaysOnTop(DEFAULT_SETTINGS.alwaysOnTop);

      setSettings(DEFAULT_SETTINGS);
      setTab("macros");
      launchWindowPlacementDone.current = false;
    } catch (err) {
      console.error("Failed to reset settings:", err);
    }
  };

  return (
    <I18nProvider language={settings.language}>
      <div className="app-root" data-tab={tab}>
        <TitleBar
          tab={tab}
          setTab={handleTabChange}
          running={status.running}
          stopReason={null}
          stopKey={0}
          isAlwaysOnTop={settings.alwaysOnTop}
          onToggleAlwaysOnTop={handleToggleAlwaysOnTop}
          onRequestClose={handleWindowClose}
        />
        <main className="panel-area">
          {tab === "macros" && (
            <MacrosPanel settings={settings} update={updateSettings} />
          )}
          {tab === "settings" && (
            <SettingsPanel
              settings={settings}
              update={updateSettings}
              running={status.running}
              appInfo={appInfo}
              onSavePreset={() => false}
              onApplyPreset={() => false}
              onUpdatePreset={() => false}
              onRenamePreset={() => false}
              onDeletePreset={() => false}
              onToggleAlwaysOnTop={handleToggleAlwaysOnTop}
              onReset={handleResetSettings}
              updateStatus={updateStatus}
              availableUpdate={availableUpdate}
              updateError={updateError}
              onCheckForUpdate={() => handleCheckForUpdate(true)}
              onApplyUpdate={handleApplyUpdate}
            />
          )}
        </main>
      </div>
    </I18nProvider>
  );
}
