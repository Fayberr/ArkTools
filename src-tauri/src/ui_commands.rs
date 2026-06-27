use std::sync::atomic::Ordering;
use tauri::AppHandle;
use tauri::Manager;

use crate::hotkeys::get_process_name_by_pid;
use windows_sys::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

use crate::app_state::AppInfoPayload;
use crate::app_state::ClickerStatusPayload;
use crate::app_state::PositionPayload;
use crate::settings::ClickerSettings;
use crate::ClickerState;

use crate::engine::mouse::current_cursor_position;
use crate::engine::worker::now_epoch_ms;
use crate::hotkeys::register_hotkey_inner;

#[tauri::command]
pub fn get_text_scale_factor() -> f64 {
    #[cfg(target_os = "windows")]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey(r"Software\Microsoft\Accessibility").ok();

        if let Some(key) = key {
            let value: u32 = key.get_value("TextScaleFactor").unwrap_or(100);
            return value as f64 / 100.0;
        }
    }

    1.0
}

#[tauri::command]
pub fn set_webview_zoom(window: tauri::Window, factor: f64) -> Result<(), String> {
    window
        .get_webview_window("main")
        .ok_or("webview not found".to_string())?
        .set_zoom(factor)
        .map_err(|e: tauri::Error| e.to_string())
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    settings: ClickerSettings,
) -> Result<ClickerSettings, String> {
    let state = app.state::<ClickerState>();
    let was_initialized = state.settings_initialized.load(Ordering::SeqCst);

    *state.settings.lock().unwrap() = settings.clone();
    crate::telemetry::track_event("settings_updated", None);

    // Re-register hotkeys from updated settings
    let mut new_bindings = Vec::new();
    log::info!("[Settings] Received {} macros", settings.macros.len());
    for m in &settings.macros {
        log::info!(
            "[Settings] Macro: id={}, enabled={}, hotkey='{}', open_key='{}', delay={}, click_pos={:?}",
            m.id, m.enabled, m.hotkey, m.open_key, m.open_key_delay_ms, m.click_position
        );
        if m.enabled && !m.hotkey.is_empty() {
            if let Ok(binding) = crate::hotkeys::parse_hotkey_binding(&m.hotkey) {
                log::info!("[Settings] Registered hotkey for macro: id={}", m.id);
                new_bindings.push((m.id.clone(), binding));
            } else {
                log::warn!(
                    "[Settings] Failed to parse hotkey for macro: id={}, hotkey='{}'",
                    m.id,
                    m.hotkey
                );
            }
        }
    }
    *state.registered_hotkeys.lock().unwrap() = new_bindings;

    if !was_initialized {
        state.settings_initialized.store(true, Ordering::SeqCst);
    }

    Ok(settings)
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<ClickerSettings, String> {
    let state = app.state::<ClickerState>();
    let settings = state.settings.lock().unwrap().clone();
    Ok(settings)
}

#[tauri::command]
pub fn reset_settings(app: AppHandle) -> Result<ClickerSettings, String> {
    let defaults = ClickerSettings::default();
    {
        let state = app.state::<ClickerState>();
        *state.settings.lock().unwrap() = defaults.clone();
        *state.registered_hotkeys.lock().unwrap() = Vec::new();
    }
    Ok(defaults)
}

#[tauri::command]
pub fn get_status() -> Result<ClickerStatusPayload, String> {
    Ok(ClickerStatusPayload {
        running: false,
        click_count: 0,
        last_error: None,
        stop_reason: None,
        active_sequence_index: None,
        active_sequence_tick: 0,
    })
}

#[tauri::command]
pub fn register_hotkey(app: AppHandle, hotkey: String) -> Result<String, String> {
    register_hotkey_inner(&app, hotkey)
}

#[tauri::command]
pub fn set_hotkey_capture_active(app: AppHandle, active: bool) -> Result<(), String> {
    let state = app.state::<ClickerState>();
    state.hotkey_capture_active.store(active, Ordering::SeqCst);

    if active {
        state
            .suppress_hotkey_until_ms
            .store(now_epoch_ms().saturating_add(250), Ordering::SeqCst);
    } else {
        state
            .suppress_hotkey_until_release
            .store(true, Ordering::SeqCst);
    }

    Ok(())
}

#[tauri::command]
pub fn pick_position() -> Result<PositionPayload, String> {
    let (x, y) =
        current_cursor_position().ok_or_else(|| String::from("Failed to read cursor position"))?;
    Ok(PositionPayload { x, y })
}

#[tauri::command]
pub fn start_sequence_point_pick(app: AppHandle) -> Result<(), String> {
    crate::sequence_picker::start_sequence_point_pick_inner(app)
}

#[tauri::command]
pub fn cancel_sequence_point_pick(app: AppHandle) -> Result<(), String> {
    crate::sequence_picker::cancel_sequence_point_pick_inner(&app);
    Ok(())
}

#[tauri::command]
pub fn get_app_info(app: AppHandle) -> Result<AppInfoPayload, String> {
    let version = app.package_info().version.to_string();
    Ok(AppInfoPayload {
        version,
        update_status: String::from("Update checks are disabled"),
        screenshot_protection_supported: false,
    })
}

#[tauri::command]
pub fn get_autostart_enabled() -> bool {
    crate::autostart::get_autostart_enabled()
}

#[tauri::command]
pub fn set_autostart_enabled(enabled: bool) -> Result<(), String> {
    crate::autostart::set_autostart_enabled(enabled).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::overlay::OVERLAY_THREAD_RUNNING.store(false, std::sync::atomic::Ordering::SeqCst);
    app.exit(0);
}

fn get_pixel_color(x: i32, y: i32) -> Option<(u8, u8, u8)> {
    unsafe {
        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return None;
        }
        let color = GetPixel(hdc, x, y);
        ReleaseDC(std::ptr::null_mut(), hdc);

        if color == 0xFFFFFFFF {
            return None;
        }

        let r = (color & 0x000000FF) as u8;
        let g = ((color & 0x0000FF00) >> 8) as u8;
        let b = ((color & 0x00FF0000) >> 16) as u8;
        Some((r, g, b))
    }
}

fn color_distance(c1: (u8, u8, u8), c2: (u8, u8, u8)) -> u32 {
    let dr = (c1.0 as i32 - c2.0 as i32).abs() as u32;
    let dg = (c1.1 as i32 - c2.1 as i32).abs() as u32;
    let db = (c1.2 as i32 - c2.2 as i32).abs() as u32;
    dr + dg + db
}

#[tauri::command]
pub fn run_macro(app: AppHandle, macro_id: String) -> Result<(), String> {
    if macro_id == "hold_e" {
        let currently_active = crate::hotkeys::AUTO_HOLD_E_ACTIVE.load(Ordering::SeqCst);
        if currently_active {
            crate::hotkeys::stop_hold_e();
            crate::telemetry::track_event("macro_stop", Some("hold_e".to_string()));
        } else {
            log::info!("[Macro] Starting Hold E");
            crate::hotkeys::AUTO_HOLD_E_ACTIVE.store(true, Ordering::SeqCst);
            crate::engine::keyboard::send_key_event(0x45, 0); // E down
            crate::telemetry::track_event("macro_start", Some("hold_e".to_string()));
        }
        return Ok(());
    }

    if macro_id == "anti_afk" {
        let currently_active = crate::hotkeys::ANTI_AFK_ACTIVE.load(Ordering::SeqCst);
        if currently_active {
            log::info!("[Macro] Stopping Anti AFK");
            crate::hotkeys::ANTI_AFK_ACTIVE.store(false, Ordering::SeqCst);
            crate::telemetry::track_event("macro_stop", Some("anti_afk".to_string()));
        } else {
            log::info!("[Macro] Starting Anti AFK");
            crate::hotkeys::ANTI_AFK_ACTIVE.store(true, Ordering::SeqCst);
            crate::telemetry::track_event("macro_start", Some("anti_afk".to_string()));
        }
        return Ok(());
    }

    let state = app.state::<ClickerState>();

    let (open_key, open_delay, click_pos, smart_take_all, auto_close_on_fail) = {
        let settings = state.settings.lock().unwrap();
        let m = settings
            .macros
            .iter()
            .find(|m| m.id == macro_id)
            .ok_or_else(|| format!("Macro with ID '{}' not found", macro_id))?;

        let click_pos = m
            .click_position
            .clone()
            .ok_or_else(|| "Calibrate this macro first".to_string())?;

        (
            m.open_key.clone(),
            m.open_key_delay_ms,
            click_pos,
            m.smart_take_all,
            m.auto_close_on_fail,
        )
    };

    crate::telemetry::track_event("macro_start", Some(macro_id.clone()));

    let (vk, _) = crate::hotkeys::parse_hotkey_main_key(&open_key, &open_key)
        .map_err(|e| format!("Failed to parse open key: {}", e))?;

    // 1. Capture color before opening inventory (representing game world color)
    let color_before = get_pixel_color(click_pos.x, click_pos.y);
    log::info!(
        "[Macro] Pixel color before inventory opens: {:?}",
        color_before
    );

    // 2. Send inventory open key (F)
    crate::engine::keyboard::send_key_event(vk as u16, 0);
    std::thread::sleep(std::time::Duration::from_millis(20));
    crate::engine::keyboard::send_key_event(
        vk as u16,
        windows_sys::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP,
    );

    // 3. Wait for inventory UI to load
    if smart_take_all && color_before.is_some() {
        let base_color = color_before.unwrap();
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_millis(1500); // 1.5s max inventory lag grace period
        let mut opened = false;

        while start_time.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(20));
            if let Some(curr_color) = get_pixel_color(click_pos.x, click_pos.y) {
                let dist = color_distance(base_color, curr_color);
                if dist > 45 {
                    log::info!(
                        "[Macro] Inventory detected as open (color changed from {:?} to {:?} after {}ms)",
                        base_color, curr_color, start_time.elapsed().as_millis()
                    );
                    opened = true;
                    // Additional short sleep to let UI fully settle and register clicks
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    break;
                }
            }
        }
        if !opened {
            log::warn!("[Macro] Timeout waiting for color change, using fallback delay.");
            std::thread::sleep(std::time::Duration::from_millis(open_delay as u64));
        }
    } else {
        std::thread::sleep(std::time::Duration::from_millis(open_delay as u64));
    }

    // 4. Move mouse to button and capture UI button color (ensures hover/highlight state is active and captured)
    crate::engine::mouse::move_mouse(click_pos.x, click_pos.y);
    std::thread::sleep(std::time::Duration::from_millis(60));
    let color_ui = get_pixel_color(click_pos.x, click_pos.y);
    log::info!("[Macro] Button color in UI (hovered): {:?}", color_ui);

    // 5. Click button. Retry up to 3 times if inventory remains open (color matches UI button)
    let mut success = false;
    let max_clicks = if smart_take_all { 3 } else { 1 };

    for attempt in 1..=max_clicks {
        if attempt > 1 {
            log::info!(
                "[Macro] Inventory still open, retrying click (attempt {})",
                attempt
            );
            crate::engine::mouse::move_mouse(click_pos.x, click_pos.y);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        crate::engine::mouse::send_mouse_event(
            windows_sys::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
        );
        std::thread::sleep(std::time::Duration::from_millis(15));
        crate::engine::mouse::send_mouse_event(
            windows_sys::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTUP,
        );

        if smart_take_all && color_ui.is_some() {
            // Wait for inventory closure animations and item transfer (usually ~100-150ms)
            std::thread::sleep(std::time::Duration::from_millis(180));

            if let Some(curr_color) = get_pixel_color(click_pos.x, click_pos.y) {
                let dist_from_ui = color_distance(color_ui.unwrap(), curr_color);
                if dist_from_ui > 45 {
                    log::info!(
                        "[Macro] Inventory closed (color changed from UI color {:?} to {:?} after click)",
                        color_ui.unwrap(), curr_color
                    );
                    success = true;
                    break;
                }
            }
        } else {
            success = true;
            break;
        }
    }

    // 6. Optional close key (F) on fail
    if auto_close_on_fail && !success && color_ui.is_some() {
        if let Some(curr_color) = get_pixel_color(click_pos.x, click_pos.y) {
            let dist_from_ui = color_distance(color_ui.unwrap(), curr_color);
            if dist_from_ui <= 45 {
                log::info!(
                    "[Macro] Inventory failed to close. Pressing close key (F) to clean up."
                );
                crate::engine::keyboard::send_key_event(vk as u16, 0);
                std::thread::sleep(std::time::Duration::from_millis(20));
                crate::engine::keyboard::send_key_event(
                    vk as u16,
                    windows_sys::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP,
                );
            }
        }
    }

    crate::telemetry::track_event("macro_stop", Some(macro_id));
    Ok(())
}

#[derive(serde::Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub title: String,
    pub process_name: String,
}

unsafe extern "system" fn enum_window_callback(
    hwnd: windows_sys::Win32::Foundation::HWND,
    lparam: isize,
) -> i32 {
    let list = &mut *(lparam as *mut Vec<WindowInfo>);
    if IsWindowVisible(hwnd) != 0 {
        let mut buffer = [0u16; 512];
        let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if len > 0 {
            let title = String::from_utf16_lossy(&buffer[..len as usize]);
            let title_trimmed = title.trim();
            if !title_trimmed.is_empty() {
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                if pid > 0 {
                    if let Some(proc_name) = get_process_name_by_pid(pid) {
                        list.push(WindowInfo {
                            title: title_trimmed.to_string(),
                            process_name: proc_name,
                        });
                    }
                }
            }
        }
    }
    1
}

#[tauri::command]
pub fn get_open_windows() -> Result<Vec<WindowInfo>, String> {
    let mut list: Vec<WindowInfo> = Vec::new();
    unsafe {
        EnumWindows(
            Some(enum_window_callback),
            &mut list as *mut Vec<WindowInfo> as isize,
        );
    }

    let ignore_processes = [
        "explorer.exe",
        "taskhostw.exe",
        "svchost.exe",
        "SearchHost.exe",
        "StartMenuExperienceHost.exe",
        "TextInputHost.exe",
        "SystemSettings.exe",
        "ApplicationFrameHost.exe",
        "Widgets.exe",
        "ShellExperienceHost.exe",
        "ArkTools.exe",
    ];
    list.retain(|w| {
        let name_lower = w.process_name.to_lowercase();
        !ignore_processes
            .iter()
            .any(|&p| p.to_lowercase() == name_lower)
    });

    list.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    Ok(list)
}
