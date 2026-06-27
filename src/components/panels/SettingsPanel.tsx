import "./SettingsPanel.css";
import type {
  AppInfo,
  Settings,
} from "../../store";
import { DEFAULT_ACCENT_COLOR } from "../../settingsSchema";
import {
  useTranslation,
} from "../../i18n";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState, useMemo, type ReactNode } from "react";
import ConfirmDialog from "../ConfirmDialog";
import { AdvDropdown } from "./advanced/shared";
// import { changelogEntries } from "../../changelog";
// import ChangelogContent from "../ChangelogContent";

type PendingAction =
  | "reset-settings"
  | null;

// const LANGUAGE_DROPDOWN_OPTIONS = LANGUAGE_OPTIONS.map((option) => ({
//   value: option.code,
//   label: option.label,
// }));

interface Props {
  settings: Settings;
  update: (patch: Partial<Settings>) => void;
  running: boolean;
  appInfo: AppInfo;
  onSavePreset: () => boolean;
  onApplyPreset: () => boolean;
  onUpdatePreset: () => boolean;
  onRenamePreset: () => boolean;
  onDeletePreset: () => boolean;
  onToggleAlwaysOnTop: () => Promise<void>;
  onReset: () => Promise<void>;
  updateStatus: "idle" | "checking" | "up-to-date" | "available" | "downloading" | "error";
  availableUpdate: { version: string } | null;
  updateError: string | null;
  onCheckForUpdate: () => Promise<void>;
  onApplyUpdate: () => Promise<void>;
}

function SettingsSectionHeading({
  title,
  description,
}: {
  title: string;
  description?: string;
}) {
  return (
    <div className="settings-section-heading">
      <span className="settings-section-title">{title}</span>
      {description && (
        <span className="settings-section-description">{description}</span>
      )}
    </div>
  );
}

function SettingsCard({
  title,
  description,
  children,
}: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section className="settings-card">
      <SettingsSectionHeading title={title} description={description} />
      <div className="settings-card-content">{children}</div>
    </section>
  );
}

interface WindowInfo {
  title: string;
  processName: string;
}

export default function SettingsPanel({
  settings,
  update,
  running,
  // appInfo,
  onToggleAlwaysOnTop,
  onReset,
  updateStatus,
  availableUpdate,
  updateError,
  onCheckForUpdate,
  onApplyUpdate,
}: Props) {
  const { t } = useTranslation();
  // const [showChangelog, setShowChangelog] = useState(false);
  const [pendingAction, setPendingAction] = useState<PendingAction>(null);
  const [isAutostartEnabled, setIsAutostartEnabled] = useState(false);
  const [openWindows, setOpenWindows] = useState<WindowInfo[]>([]);

  const initialMode = settings.targetProcess
    ? "detected"
    : settings.targetWindow
    ? "custom"
    : "global";
  const [restrictionMode, setRestrictionMode] = useState<"global" | "detected" | "custom">(initialMode);

  const refreshWindows = async () => {
    try {
      const windows = await invoke<WindowInfo[]>("get_open_windows");
      setOpenWindows(windows);
    } catch (err) {
      console.error("Failed to get open windows:", err);
    }
  };

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    refreshWindows();
  }, []);

  useEffect(() => {
    invoke<boolean>("get_autostart_enabled")
      .then(setIsAutostartEnabled)
      .catch(console.error);
  }, []);

  const handleToggleAutostart = async () => {
    const nextVal = !isAutostartEnabled;
    try {
      await invoke("set_autostart_enabled", { enabled: nextVal });
      setIsAutostartEnabled(nextVal);
      update({ autostart: nextVal });
    } catch (err) {
      console.error("Failed to toggle autostart:", err);
    }
  };

  const handleToggleTheme = () => {
    update({ theme: settings.theme === "dark" ? "light" : "dark" });
  };

  const handleToggleMinimizeToTray = () => {
    update({ minimizeToTray: !settings.minimizeToTray });
  };

  const handleConfirmReset = async () => {
    setPendingAction(null);
    await onReset();
    setIsAutostartEnabled(false);
  };

  const detectedOptions = useMemo(() => {
    const list = openWindows.map((w) => ({
      value: `${w.processName}|${w.title}`,
      label: `${w.title} (${w.processName})`,
    }));

    if (
      settings.targetProcess &&
      !openWindows.some((w) => w.processName === settings.targetProcess)
    ) {
      list.unshift({
        value: `${settings.targetProcess}|${settings.targetWindow}`,
        label: `${settings.targetWindow || "Selected Window"} (${settings.targetProcess}) [Not Running]`,
      });
    }

    return list;
  }, [openWindows, settings.targetProcess, settings.targetWindow]);

  return (
    <div className="settings-panel">
      <h2 className="panel-title">{t("titleBar.settings")}</h2>

      <div className="settings-cards">
        {/* About Section */}
        {/*
        <SettingsCard
          title={t("settings.sectionAbout")}
          description={t("settings.sectionAboutDescription")}
        >
          <div className="about-grid">
            <div className="about-row">
              <span className="about-label">{t("settings.version")}</span>
              <span className="about-value">{appInfo.version}</span>
            </div>
            <div className="about-row">
              <button
                className="changelog-toggle-btn"
                onClick={() => setShowChangelog(!showChangelog)}
              >
                {showChangelog ? t("settings.hideChanges") : t("settings.showChanges")}
              </button>
            </div>
          </div>
          {showChangelog && (
            <div className="changelog-container">
              <ChangelogContent entries={changelogEntries} />
            </div>
          )}
        </SettingsCard>
        */}

        {/* Behavior */}
        <SettingsCard
          title={t("settings.sectionBehavior")}
          description={t("settings.sectionBehaviorDescription")}
        >
          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.alwaysOnTop")}</span>
              <span className="settings-sublabel">
                {t("settings.alwaysOnTopDescription")}
              </span>
            </div>
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.alwaysOnTop}
                onChange={onToggleAlwaysOnTop}
              />
              <span className="slider"></span>
            </label>
          </div>

          <div className="settings-row settings-row--align-center">
            <div className="settings-label-group">
              <span className="settings-label">Target Window Restriction</span>
              <span className="settings-sublabel">
                Only trigger macro keys when a specific window is active.
              </span>
            </div>
            <div className="settings-control-width-md">
              <AdvDropdown
                options={[
                  { value: "global", label: "Any Window (Global)" },
                  { value: "detected", label: "Select Running Game..." },
                  { value: "custom", label: "Custom Window Title..." },
                ]}
                value={restrictionMode}
                onChange={(mode) => {
                  setRestrictionMode(mode as "global" | "detected" | "custom");
                  if (mode === "global") {
                    update({ targetProcess: "", targetWindow: "" });
                  } else if (mode === "custom") {
                    update({ targetProcess: "", targetWindow: settings.targetWindow || "ARK" });
                  } else if (mode === "detected") {
                    if (openWindows.length > 0) {
                      update({
                        targetProcess: openWindows[0].processName,
                        targetWindow: openWindows[0].title,
                      });
                    } else {
                      update({ targetProcess: "", targetWindow: "" });
                    }
                  }
                }}
              />
            </div>
          </div>

          {restrictionMode === "detected" && (
            <div className="settings-row-nested">
              <div className="settings-label-group">
                <span className="settings-label">Select Game Window</span>
                <span className="settings-sublabel">
                  List of open window titles and process executables.
                </span>
              </div>
              <div className="settings-control-container settings-control-width-lg">
                <div style={{ flex: 1, minWidth: 0 }}>
                  <AdvDropdown
                    options={detectedOptions}
                    value={settings.targetProcess ? `${settings.targetProcess}|${settings.targetWindow}` : ""}
                    onChange={(val) => {
                      if (!val) return;
                      const [proc, title] = val.split("|");
                      update({ targetProcess: proc, targetWindow: title });
                    }}
                    placeholder="No windows detected"
                  />
                </div>
                <button
                  type="button"
                  className="settings-btn-quiet settings-btn-icon"
                  onClick={refreshWindows}
                  title="Refresh open windows list"
                >
                  ↻
                </button>
              </div>
            </div>
          )}

          {restrictionMode === "custom" && (
            <div className="settings-row-nested">
              <div className="settings-label-group">
                <span className="settings-label">Custom Window Title Search</span>
                <span className="settings-sublabel">
                  Keyword query matching the window title bar.
                </span>
              </div>
              <input
                type="text"
                value={settings.targetWindow}
                onChange={(e) => update({ targetWindow: e.target.value })}
                className="text-input settings-control-width-md"
                placeholder="e.g. ARK"
              />
            </div>
          )}
        </SettingsCard>

        {/* Startup */}
        <SettingsCard
          title={t("settings.sectionStartup")}
          description={t("settings.sectionStartupDescription")}
        >
          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.runOnStartup")}</span>
              <span className="settings-sublabel">
                Automatically start ArkTools with Windows.
              </span>
            </div>
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={isAutostartEnabled}
                onChange={handleToggleAutostart}
                disabled={running}
              />
              <span className="slider"></span>
            </label>
          </div>

          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.minimizeToTray")}</span>
              <span className="settings-sublabel">
                {t("settings.minimizeToTrayDescription")}
              </span>
            </div>
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.minimizeToTray}
                onChange={handleToggleMinimizeToTray}
              />
              <span className="slider"></span>
            </label>
          </div>
        </SettingsCard>

        {/* Appearance */}
        <SettingsCard
          title={t("settings.sectionAppearance")}
          description={t("settings.sectionAppearanceDescription")}
        >
          {/*
          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.language")}</span>
              <span className="settings-sublabel">
                {t("settings.languageDescription")}
              </span>
            </div>
            <div className="settings-control-width-sm">
              <AdvDropdown
                options={LANGUAGE_DROPDOWN_OPTIONS}
                value={settings.language}
                onChange={(lang) => update({ language: lang as any })}
              />
            </div>
          </div>
          */}

          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">Theme</span>
              <span className="settings-sublabel">
                {t("settings.themeDescription")}
              </span>
            </div>
            <div className="settings-seg-group">
              {(["dark", "light"] as const).map((theme) => (
                <button
                  key={theme}
                  className={`settings-seg-btn ${settings.theme === theme ? "active" : ""}`}
                  onClick={handleToggleTheme}
                >
                  {t(theme === "dark" ? "common.dark" : "common.light")}
                </button>
              ))}
            </div>
          </div>

          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.accentColor")}</span>
              <span className="settings-sublabel">
                {t("settings.accentColorDescription")}
              </span>
            </div>
            <div className="settings-color-controls">
              <label className="settings-color-picker">
                <input
                  type="color"
                  value={settings.accentColor}
                  onChange={(e) => update({ accentColor: e.target.value })}
                />
              </label>
              <span className="settings-value settings-value--mono">
                {settings.accentColor.toUpperCase()}
              </span>
              <button
                type="button"
                className="settings-btn-secondary settings-btn-compact"
                onClick={() => update({ accentColor: DEFAULT_ACCENT_COLOR })}
                disabled={settings.accentColor === DEFAULT_ACCENT_COLOR}
              >
                {t("common.reset")}
              </button>
            </div>
          </div>
        </SettingsCard>

        {/* Software Updates */}
        <SettingsCard
          title={t("settings.sectionUpdates")}
          description={t("settings.sectionUpdatesDescription")}
        >
          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.autoUpdate")}</span>
              <span className="settings-sublabel">
                {t("settings.autoUpdateDescription")}
              </span>
            </div>
            <label className="toggle-switch">
              <input
                type="checkbox"
                checked={settings.autoUpdate}
                onChange={() => update({ autoUpdate: !settings.autoUpdate })}
              />
              <span className="slider"></span>
            </label>
          </div>

          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.updateStatusLabel")}</span>
              <span className="settings-sublabel">
                {updateStatus === "idle" && t("settings.updateStatusIdle")}
                {updateStatus === "checking" && t("settings.checkingForUpdate")}
                {updateStatus === "up-to-date" && t("settings.noUpdateAvailable")}
                {updateStatus === "available" && availableUpdate && `${t("settings.updateAvailable")} (v${availableUpdate.version})`}
                {updateStatus === "downloading" && t("settings.updateDownloading")}
                {updateStatus === "error" && `${t("settings.updateCheckFailed")}${updateError ? `: ${updateError}` : ""}`}
              </span>
            </div>
            <div className="settings-row-actions">
              {updateStatus === "available" && (
                <button
                  type="button"
                  className="settings-btn-primary"
                  onClick={onApplyUpdate}
                >
                  {t("settings.installUpdate")}
                </button>
              )}
              <button
                type="button"
                className="settings-btn-secondary check-update-btn"
                onClick={onCheckForUpdate}
                disabled={updateStatus === "checking" || updateStatus === "downloading"}
              >
                {updateStatus === "checking" ? t("settings.checkingForUpdate") : t("settings.checkForUpdate")}
              </button>
            </div>
          </div>
        </SettingsCard>

        {/* Reset Section */}
        <SettingsCard
          title={t("settings.sectionReset")}
          description={t("settings.sectionResetDescription")}
        >
          <div className="settings-row">
            <div className="settings-label-group">
              <span className="settings-label">{t("settings.resetAll")}</span>
              <span className="settings-sublabel">
                {t("settings.resetAllDescription")}
              </span>
            </div>
            <button
              className="settings-btn-danger"
              onClick={() => setPendingAction("reset-settings")}
              disabled={running}
            >
              {t("common.reset")}
            </button>
          </div>
        </SettingsCard>
      </div>

      {pendingAction === "reset-settings" && (
        <ConfirmDialog
          open={true}
          title={t("settings.resetDialogTitle")}
          message={t("settings.resetDialogMessage")}
          confirmLabel={t("settings.resetDialogConfirm")}
          onConfirm={handleConfirmReset}
          onCancel={() => setPendingAction(null)}
        />
      )}
    </div>
  );
}
