import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { type Settings } from "../../settingsSchema";
import HotkeyCaptureInput from "../HotkeyCaptureInput";
import KeyCaptureInput from "../KeyCaptureInput";
import "./MacrosPanel.css";

interface Props {
  settings: Settings;
  update: (next: Partial<Settings>) => void;
}

export default function MacrosPanel({ settings, update }: Props) {
  const [calibratingMacroId, setCalibratingMacroId] = useState<string | null>(null);

  useEffect(() => {
    if (!calibratingMacroId) return;

    let unlistenPicked: (() => void) | null = null;
    let unlistenEnded: (() => void) | null = null;

    const setupListeners = async () => {
      unlistenPicked = await listen<{ x: number; y: number }>(
        "sequence-point-picked",
        (event) => {
          const { x, y } = event.payload;
          
          const updatedMacros = settings.macros.map((m) => {
            if (m.id === calibratingMacroId) {
              return {
                ...m,
                clickPosition: { x, y },
              };
            }
            return m;
          });
          
          update({ macros: updatedMacros });
          invoke("cancel_sequence_point_pick").catch(console.error);
        }
      );

      unlistenEnded = await listen<void>("sequence-pick-ended", () => {
        setCalibratingMacroId(null);
      });
    };

    setupListeners().catch(console.error);

    return () => {
      unlistenPicked?.();
      unlistenEnded?.();
    };
  }, [calibratingMacroId, settings.macros, update]);

  const handleToggleEnabled = (macroId: string, enabled: boolean) => {
    const updated = settings.macros.map((m) =>
      m.id === macroId ? { ...m, enabled } : m
    );
    update({ macros: updated });
  };

  const handleHotkeyChange = (macroId: string, hotkey: string) => {
    const updated = settings.macros.map((m) =>
      m.id === macroId ? { ...m, hotkey } : m
    );
    update({ macros: updated });
  };

  const handleOpenKeyChange = (macroId: string, openKey: string) => {
    const updated = settings.macros.map((m) =>
      m.id === macroId ? { ...m, openKey } : m
    );
    update({ macros: updated });
  };

  const handleDelayChange = (macroId: string, delayMs: number) => {
    const updated = settings.macros.map((m) =>
      m.id === macroId ? { ...m, openKeyDelayMs: Math.max(0, Math.min(10000, delayMs)) } : m
    );
    update({ macros: updated });
  };

  const handleSmartTakeAllChange = (macroId: string, smartTakeAll: boolean) => {
    const updated = settings.macros.map((m) =>
      m.id === macroId ? { ...m, smartTakeAll } : m
    );
    update({ macros: updated });
  };

  const handleAutoCloseOnFailChange = (macroId: string, autoCloseOnFail: boolean) => {
    const updated = settings.macros.map((m) =>
      m.id === macroId ? { ...m, autoCloseOnFail } : m
    );
    update({ macros: updated });
  };

  const startCalibration = async (macroId: string) => {
    try {
      setCalibratingMacroId(macroId);
      await invoke("start_sequence_point_pick");
    } catch (err) {
      console.error("Failed to start calibration:", err);
      setCalibratingMacroId(null);
    }
  };

  return (
    <div className="macros-panel">
      <h2 className="panel-title">Macros</h2>
      <div className="macros-list">
        {settings.macros.map((macro) => (
          <div key={macro.id} className="macro-card">
            <div className="macro-header">
              <span className="macro-name">{macro.name}</span>
              <label className="toggle-switch">
                <input
                  type="checkbox"
                  checked={macro.enabled}
                  onChange={(e) => handleToggleEnabled(macro.id, e.target.checked)}
                />
                <span className="slider"></span>
              </label>
            </div>

            <div className="macro-settings-grid">
              <div className="setting-field">
                <label className="setting-label">Trigger Key / Hotkey</label>
                <HotkeyCaptureInput
                  value={macro.hotkey}
                  onChange={(val) => handleHotkeyChange(macro.id, val)}
                  className="settings-input"
                />
              </div>

              {macro.id === "take_all" && (
                <>
                  <div className="setting-field">
                    <label className="setting-label">Open Key</label>
                    <KeyCaptureInput
                      value={macro.openKey}
                      onChange={(val) => handleOpenKeyChange(macro.id, val)}
                      className="settings-input"
                    />
                  </div>

                  <div className="setting-field">
                    <label className="setting-label">Delay (ms)</label>
                    <input
                      type="number"
                      value={macro.openKeyDelayMs}
                      onChange={(e) => handleDelayChange(macro.id, parseInt(e.target.value) || 0)}
                      className="settings-input number-input"
                      min={0}
                      max={10000}
                    />
                  </div>

                  <div className="setting-field calibration-field">
                    <label className="setting-label">Click Position</label>
                    <div className="calibration-controls">
                      <button
                        className={`calibrate-btn ${calibratingMacroId === macro.id ? "calibrating" : ""}`}
                        onClick={() => startCalibration(macro.id)}
                      >
                        {calibratingMacroId === macro.id ? "Calibrating..." : "Calibrate"}
                      </button>
                      <span className="position-coords">
                        {macro.clickPosition
                          ? `(${macro.clickPosition.x}, ${macro.clickPosition.y})`
                          : "Not calibrated"}
                      </span>
                    </div>
                  </div>

                  <div className="setting-field checkbox-field" style={{ gridColumn: "span 2", display: "flex", flexWrap: "wrap", gap: "1.25rem 2rem", marginTop: "4px" }}>
                    <label className="checkbox-label" style={{ display: "flex", alignItems: "center", gap: "0.5rem", cursor: "pointer", fontSize: "0.8125rem", color: "var(--text-muted)", userSelect: "none" }}>
                      <input
                        type="checkbox"
                        checked={macro.smartTakeAll ?? true}
                        onChange={(e) => handleSmartTakeAllChange(macro.id, e.target.checked)}
                        style={{ cursor: "pointer", width: "14px", height: "14px", accentColor: "var(--accent-green)" }}
                      />
                      Smart Lag Check (Checks color before/after click, retries up to 3x)
                    </label>

                    <label className="checkbox-label" style={{ display: "flex", alignItems: "center", gap: "0.5rem", cursor: "pointer", fontSize: "0.8125rem", color: "var(--text-muted)", userSelect: "none" }}>
                      <input
                        type="checkbox"
                        checked={macro.autoCloseOnFail ?? false}
                        onChange={(e) => handleAutoCloseOnFailChange(macro.id, e.target.checked)}
                        style={{ cursor: "pointer", width: "14px", height: "14px", accentColor: "var(--accent-green)" }}
                      />
                      Auto-close Inventory on Fail (Presses F if inventory fails to close)
                    </label>
                  </div>
                </>
              )}

              {macro.id === "auto_walk" && (
                <>
                  <div className="setting-field checkbox-field" style={{ gridColumn: "span 2", display: "flex", flexWrap: "wrap", gap: "1.25rem 2rem", marginTop: "4px" }}>
                    <label className="checkbox-label" style={{ display: "flex", alignItems: "center", gap: "0.5rem", cursor: "pointer", fontSize: "0.8125rem", color: "var(--text-muted)", userSelect: "none" }}>
                      <input
                        type="checkbox"
                        checked={macro.holdSprint ?? false}
                        onChange={(e) => {
                          const updated = settings.macros.map((m) =>
                            m.id === macro.id ? { ...m, holdSprint: e.target.checked } : m
                          );
                          update({ macros: updated });
                        }}
                        style={{ cursor: "pointer", width: "14px", height: "14px", accentColor: "var(--accent-green)" }}
                      />
                      Hold Sprint (Shift) as well
                    </label>
                  </div>
                  <div className="setting-field" style={{ gridColumn: "span 2", fontSize: "0.8125rem", color: "var(--text-muted)", lineHeight: "1.4" }}>
                    <strong>How it works:</strong> Tapping the trigger key twice quickly (double-tap) will lock walking input. Tap the trigger key once again physically to stop walking.
                  </div>
                </>
              )}

              {macro.id === "auto_tek_legs" && (
                <div className="setting-field" style={{ gridColumn: "span 2", fontSize: "0.8125rem", color: "var(--text-muted)", lineHeight: "1.4" }}>
                  <strong>How it works:</strong> Tapping the trigger key twice quickly (double-tap) will lock Shift + Control inputs to keep your Tek Legs active. Tap the trigger key once again physically to stop.
                </div>
              )}

              {macro.id === "hold_e" && (
                <div className="setting-field" style={{ gridColumn: "span 2", fontSize: "0.8125rem", color: "var(--text-muted)", lineHeight: "1.4" }}>
                  <strong>How it works:</strong> Tapping the configured hotkey will toggle holding the E key continuously. Pressing the hotkey again or physically pressing E stops it.
                </div>
              )}

              {macro.id === "anti_afk" && (
                <div className="setting-field" style={{ gridColumn: "span 2", fontSize: "0.8125rem", color: "var(--text-muted)", lineHeight: "1.4" }}>
                  <strong>How it works:</strong> Tapping the configured hotkey will toggle Anti-AFK mode on/off. When active, it simulates realistic player movements, jump/punch combos, and camera view rotations at randomized intervals to prevent getting kicked from official and public servers.
                </div>
              )}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
