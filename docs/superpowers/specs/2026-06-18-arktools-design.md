# ArkTools — Design Spec

Date: 2026-06-18
Status: Approved

## Summary

ArkTools is a fork of Blur-AutoClicker that keeps the original app's UI shell,
theming, window chrome, and low-level input engine, but replaces the
autoclicker functionality with a small, extensible **premade macro launcher**
for the game ARK. v1 ships exactly one macro, "Take All", built on a
step-execution framework designed so additional premade macros can be added
later without re-architecting.

## Goals (v1)

- Rename/reskin the app to "ArkTools".
- Replace the autoclicker's Simple/Advanced/Zones tabs with a single
  "Macros" tab listing premade macros.
- Ship one premade macro: **Take All** — press a configurable "open" key
  (e.g. `F` to open an inventory), wait a configurable delay, then click a
  calibrated screen pixel position (the in-game "Take All" button).
- Each macro has its own configurable global hotkey (e.g. `V`) that triggers
  it once per press (no hold-to-repeat in v1).
- Screen position is taught via manual calibration: reuse the existing
  on-screen point-picker overlay so the user can click the real button once
  in-game and have its pixel coordinates captured and stored per-macro.
- Keep Settings tab (theme, accent color, language, autostart, minimize to
  tray, always-on-top, reset) but drop the multi-preset system and the
  global single-hotkey field (now per-macro).

## Non-goals (v1)

- No generic/freeform macro step editor — macros are premade, hardcoded
  step sequences in Rust; only a few parameters per macro (hotkey, open key,
  delay, calibrated click position) are user-configurable.
- No additional premade macros beyond Take All (Transfer All, Drop All,
  etc. are deferred).
- No pixel-color verification/condition checks before clicking.
- No hold-to-repeat or toggle-to-repeat run modes — single press, single
  run.
- No stats/run-count tracking.
- No auto-updater UI or signed release CI — this isn't being distributed
  yet; that infra can be reintroduced later if/when ArkTools is shared.
- Multi-language (i18n) scaffolding is kept in the codebase for future use,
  but only `en.json` strings are updated for the new Macros panel; other
  locale files are not maintained in v1 and may go stale.

## Architecture

### Kept as-is

- Tauri app shell: window chrome, `TitleBar`, theming/accent-color system,
  i18n provider scaffolding, autostart, minimize-to-tray, always-on-top,
  settings persistence under `%appdata%`.
- Low-level Windows input primitives: `SendInput`-based keyboard
  (`make_keyboard_input`, `send_key_event`) and mouse click/move functions
  in `engine/keyboard.rs` / `engine/mouse.rs`.
- Global hotkey poll loop and hotkey string parsing in `hotkeys.rs`
  (`parse_hotkey_binding`, `format_hotkey_binding`, physical-key-state
  tracking via low-level hooks).
- On-screen point-picker overlay (`sequence_picker.rs` + `overlay.rs`):
  right-click-to-pick mechanism with virtual-screen coordinate offsetting,
  Escape-to-cancel, used unchanged for macro calibration.

### Removed

- CPS/duty-cycle/speed-variation/double-click repeat-click engine
  (`engine/cycle.rs`, `engine/rng.rs`, the repeat loop in
  `engine/worker.rs`).
- Click/time limits, corner-stop, edge-stop, custom-stop-zone and their
  overlay rendering paths, `engine/failsafe.rs`.
- Simple/Advanced/Zones tabs and their components
  (`SimplePanel`, `AdvancedPanel` + its sections, `ZonesPanel` + its
  sections).
- Multi-preset system (save/apply/rename/delete preset UI and backend
  fields).
- Cumulative click-count stats (`engine/stats.rs`, `get_stats`/`reset_stats`
  commands).
- Update-checker UI (`Updatebanner.tsx`) and the signed-release GitHub
  Actions workflow.

### New

- A macro executor: `run_macro(app, macro_id)` Tauri command. Looks up the
  `MacroDefinition` for `macro_id`, returns an error if `clickPosition` is
  unset ("Calibrate this macro first"), otherwise runs synchronously:
  1. `send_key_event(openKey, down)` then `send_key_event(openKey, up)`
  2. sleep `openKeyDelayMs`
  3. move cursor to `clickPosition` and send a single left mouse click

  This bypasses `ClickCyclePlan`/`RunControl` entirely — there is no loop,
  so the repeat-click machinery isn't needed.
- Hotkey registry extended from a single `Option<HotkeyBinding>` to a list
  of `(macro_id, HotkeyBinding)` pairs. The existing poll loop in
  `hotkeys.rs` is extended to check all registered bindings each tick and
  invoke `run_macro` for whichever transitions to pressed (reusing the
  existing press/release edge-detection logic).

## Data model

```ts
interface MacroDefinition {
  id: "take_all";              // fixed set of ids, hardcoded executor logic
  name: string;                 // display name, e.g. "Take All"
  enabled: boolean;
  hotkey: string;                // e.g. "v"
  openKey: string;                // key pressed first, e.g. "f"
  openKeyDelayMs: number;         // wait before clicking, default 300
  clickPosition: { x: number; y: number } | null;  // null until calibrated
}

interface Settings {
  macros: MacroDefinition[];
  theme: Theme;
  accentColor: string;
  language: Language;
  minimizeToTray: boolean;
  alwaysOnTop: boolean;
  autostart: boolean;
  lastPanel: "macros" | "settings";
}
```

The Rust-side `MacroDefinition` struct mirrors this. The macro's *step
sequence* (which keys/waits/clicks happen, in what order) is hardcoded per
`id` inside the executor — only the four configurable fields above
(`hotkey`, `openKey`, `openKeyDelayMs`, `clickPosition`) are
user-editable. Adding a future premade macro means adding a new `id` match
arm to the executor and a corresponding card in the UI, not building a
generic editor.

## Frontend changes

- `App.tsx`: two tabs — **Macros**, **Settings**. Window-size logic
  simplified since there's no longer a Simple/Advanced/Zones size matrix.
- New `MacrosPanel` component: renders one card per `MacroDefinition`.
  Each card has:
  - enable/disable toggle
  - macro name (static label, not user-editable)
  - hotkey-capture input (reuses `HotkeyCaptureInput.tsx`)
  - "Calibrate" button — shows current `(x, y)` or "Not calibrated";
    clicking it starts the point-picker overlay, listens for
    `sequence-point-picked`, stores the result, then cancels picking
  - `openKey` capture (reuses `KeyCaptureInput.tsx`) and
    `openKeyDelayMs` number input (reuses `numberInput.ts` validation)
- `SettingsPanel`: drop preset save/apply/rename/delete sections and the
  global hotkey field; keep theme, accent color, language, autostart,
  minimize-to-tray, always-on-top, reset-to-defaults.
- `store.ts` / `settingsSchema.ts`: rewritten field list — `macros[]`
  replaces all click-speed/duty-cycle/sequence/zone fields; preset-related
  types and constants (`MAX_PRESETS`, `PresetId`, etc.) removed.

## Branding

- Product renamed to "ArkTools" in `package.json`, `tauri.conf.json`,
  `Cargo.toml`, window title, and README.
- Existing Blur `.ico`/logo files kept as a placeholder; swapping in custom
  ArkTools branding is an explicit future cosmetic pass, not part of this
  spec.
- Update-checker and signed-release CI removed/disabled (see Non-goals).

## Open extension points (explicitly deferred, not built now)

- `runMode` field on `MacroDefinition` (Once / Hold-to-repeat /
  Toggle-to-repeat) — data model already supports adding this later.
- Additional premade macro `id`s (Transfer All, Drop All, etc.).
- Pixel-color verification step before clicking.
- Run-count/stats tracking per macro.
- Re-enabling auto-updater/signed releases if the app is later distributed.
