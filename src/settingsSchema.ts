import { DEFAULT_LANGUAGE, isLanguage, type Language } from "./i18n";

export type Theme = "dark" | "light";
export const THEME_OPTIONS = ["dark", "light"] as const satisfies ReadonlyArray<Theme>;
export const DEFAULT_ACCENT_COLOR = "#22c55e";

export interface MacroDefinition {
  id: string;
  name: string;
  enabled: boolean;
  hotkey: string;
  openKey: string;
  openKeyDelayMs: number;
  clickPosition: { x: number; y: number } | null;
  smartTakeAll: boolean;
  autoCloseOnFail: boolean;
  holdSprint?: boolean;
}


export interface Settings {
  version: string;
  macros: MacroDefinition[];
  theme: Theme;
  accentColor: string;
  language: Language;
  minimizeToTray: boolean;
  alwaysOnTop: boolean;
  autostart: boolean;
  lastPanel: "macros" | "settings";
  targetWindow: string;
  targetProcess: string;
  autoUpdate: boolean;
}

export function createDefaultSettings(version: string): Settings {
  return {
    version,
    macros: [
      {
        id: "take_all",
        name: "Take All",
        enabled: false,
        hotkey: "",
        openKey: "f",
        openKeyDelayMs: 300,
        clickPosition: null,
        smartTakeAll: true,
        autoCloseOnFail: false,
      },
      {
        id: "auto_walk",
        name: "Auto Walk",
        enabled: false,
        hotkey: "KeyW",
        openKey: "",
        openKeyDelayMs: 0,
        clickPosition: null,
        smartTakeAll: false,
        autoCloseOnFail: false,
        holdSprint: false,
      },
      {
        id: "auto_tek_legs",
        name: "Auto Tek Legs",
        enabled: false,
        hotkey: "ctrl",
        openKey: "",
        openKeyDelayMs: 0,
        clickPosition: null,
        smartTakeAll: false,
        autoCloseOnFail: false,
      },
      {
        id: "hold_e",
        name: "Hold E",
        enabled: false,
        hotkey: "",
        openKey: "",
        openKeyDelayMs: 0,
        clickPosition: null,
        smartTakeAll: false,
        autoCloseOnFail: false,
      },
      {
        id: "anti_afk",
        name: "Anti AFK",
        enabled: false,
        hotkey: "",
        openKey: "",
        openKeyDelayMs: 0,
        clickPosition: null,
        smartTakeAll: false,
        autoCloseOnFail: false,
      },
    ],
    theme: "dark",
    accentColor: DEFAULT_ACCENT_COLOR,
    language: DEFAULT_LANGUAGE as Language,
    minimizeToTray: false,
    alwaysOnTop: false,
    autostart: false,
    lastPanel: "macros",
    targetWindow: "",
    targetProcess: "",
    autoUpdate: true,
  };
}

export function clampNumber(
  value: unknown,
  fallback: number,
  min?: number,
  max?: number,
) {
  const parsed =
    typeof value === "number" && Number.isFinite(value) ? value : fallback;
  const minClamped = min === undefined ? parsed : Math.max(min, parsed);
  return max === undefined ? minClamped : Math.min(max, minClamped);
}

export function sanitizeBoolean(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

export function sanitizeHexColor(value: unknown, fallback: string): string {
  if (typeof value !== "string") {
    return fallback;
  }
  const normalized = value.trim().toLowerCase();
  return /^#[0-9a-f]{6}$/.test(normalized) ? normalized : fallback;
}

function sanitizeEnum<T extends string>(
  value: unknown,
  fallback: T,
  valid: readonly T[],
): T {
  return typeof value === "string" && valid.includes(value as T)
    ? (value as T)
    : fallback;
}

function sanitizeMacros(value: unknown): MacroDefinition[] {
  if (!Array.isArray(value)) {
    return createDefaultSettings("").macros;
  }
  
  const defaultMacros = createDefaultSettings("").macros;
  
  return defaultMacros.map((def) => {
    const matched = value.find((m) => m && typeof m === "object" && m.id === def.id);
    if (!matched) return def;
    
    const x = typeof matched.clickPosition?.x === "number" ? Math.trunc(matched.clickPosition.x) : null;
    const y = typeof matched.clickPosition?.y === "number" ? Math.trunc(matched.clickPosition.y) : null;
    const clickPosition = (x !== null && y !== null) ? { x, y } : null;

    return {
      id: def.id,
      name: def.name,
      enabled: sanitizeBoolean(matched.enabled, def.enabled),
      hotkey: typeof matched.hotkey === "string" ? matched.hotkey : def.hotkey,
      openKey: typeof matched.openKey === "string" ? matched.openKey : def.openKey,
      openKeyDelayMs: clampNumber(matched.openKeyDelayMs, def.openKeyDelayMs, 0, 10000),
      clickPosition,
      smartTakeAll: sanitizeBoolean(matched.smartTakeAll, def.smartTakeAll),
      autoCloseOnFail: sanitizeBoolean(matched.autoCloseOnFail, def.autoCloseOnFail),
      holdSprint: sanitizeBoolean(matched.holdSprint, def.holdSprint ?? false),
    };
  });
}

export function sanitizeSettings(
  input: Partial<Settings> | null | undefined,
  version: string,
): Settings {
  const defaults = createDefaultSettings(version);
  const saved = (input ?? {}) as Partial<Settings>;

  return {
    version,
    macros: sanitizeMacros(saved.macros),
    theme: sanitizeEnum(saved.theme, defaults.theme, THEME_OPTIONS),
    accentColor: sanitizeHexColor(saved.accentColor, defaults.accentColor),
    language: isLanguage(saved.language) ? saved.language : defaults.language,
    minimizeToTray: sanitizeBoolean(saved.minimizeToTray, defaults.minimizeToTray),
    alwaysOnTop: sanitizeBoolean(saved.alwaysOnTop, defaults.alwaysOnTop),
    autostart: sanitizeBoolean(saved.autostart, defaults.autostart),
    lastPanel: sanitizeEnum(saved.lastPanel, defaults.lastPanel, ["macros", "settings"]),
    targetWindow: typeof saved.targetWindow === "string" ? saved.targetWindow : defaults.targetWindow,
    targetProcess: typeof saved.targetProcess === "string" ? saved.targetProcess : defaults.targetProcess,
    autoUpdate: sanitizeBoolean(saved.autoUpdate, defaults.autoUpdate),
  };
}
