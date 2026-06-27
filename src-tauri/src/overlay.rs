use crate::engine::mouse::{
    current_cursor_position, current_virtual_screen_rect, VirtualScreenRect,
};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};

static SEQUENCE_PICK_OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static OVERLAY_THREAD_RUNNING: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowLongW, SetWindowLongW, SetWindowPos, ShowWindow, GWL_EXSTYLE, GWL_STYLE,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMNCRP_DISABLED};

pub fn init_overlay(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| "Overlay window not found".to_string())?;

    log::info!("[Overlay] Running one-time init...");

    window
        .set_ignore_cursor_events(true)
        .map_err(|e| e.to_string())?;
    let _ = window.set_decorations(false);

    #[cfg(target_os = "windows")]
    {
        apply_win32_styles(&window)?;
        let _ = sync_overlay_bounds(&window)?;
    }

    log::info!("[Overlay] Init complete — window configured but hidden");
    Ok(())
}

pub fn show_sequence_pick_overlay(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("overlay")
        .ok_or_else(|| "Overlay window not found".to_string())?;
    let bounds = current_virtual_screen_rect()
        .ok_or_else(|| "Virtual screen bounds not available".to_string())?;

    #[cfg(target_os = "windows")]
    {
        sync_overlay_bounds(&window)?;
        show_overlay_window(&window)?;
    }

    SEQUENCE_PICK_OVERLAY_ACTIVE.store(true, Ordering::SeqCst);
    set_sequence_pick_mode(app, true)?;

    if let Some((x, y)) = current_cursor_position() {
        let offset = VirtualScreenRect::new(x, y, 1, 1).offset_from(bounds);
        let _ = window.emit(
            "sequence-pick-cursor",
            serde_json::json!({
                "x": offset.left,
                "y": offset.top,
            }),
        );
    }

    Ok(())
}

pub fn set_sequence_pick_mode(app: &AppHandle, active: bool) -> Result<(), String> {
    SEQUENCE_PICK_OVERLAY_ACTIVE.store(active, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = window.emit(
            "sequence-pick-mode",
            serde_json::json!({
                "active": active,
            }),
        );
    }
    Ok(())
}

#[tauri::command]
pub fn hide_overlay(app: AppHandle) -> Result<(), String> {
    SEQUENCE_PICK_OVERLAY_ACTIVE.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("overlay") {
        hide_overlay_window(&window);
    }
    Ok(())
}

fn hide_overlay_window(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "windows")]
    {
        if let Ok(hwnd) = get_hwnd(window) {
            unsafe { ShowWindow(hwnd, 0) };
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = window.hide();
}

#[cfg(target_os = "windows")]
fn get_hwnd(window: &tauri::WebviewWindow) -> Result<*mut std::ffi::c_void, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    match handle.as_raw() {
        RawWindowHandle::Win32(w) => Ok(w.hwnd.get() as *mut std::ffi::c_void),
        _ => Err("Not a Win32 window".to_string()),
    }
}

#[cfg(target_os = "windows")]
fn apply_win32_styles(window: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = get_hwnd(window)?;

    unsafe {
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        SetWindowLongW(hwnd, GWL_STYLE, ((style as u32) | 0x8000_0000) as i32);

        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE);
        let new_ex =
            ((ex as u32) | 0x0800_0000 | 0x0000_0080 | 0x0000_0020 | 0x0000_0008) & !0x0004_0000;
        SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex as i32);

        let policy = DWMNCRP_DISABLED;
        DwmSetWindowAttribute(
            hwnd,
            2,
            &policy as *const i32 as *const _,
            std::mem::size_of::<i32>() as u32,
        );

        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }

    log::info!("[Overlay] Win32 styles applied");
    Ok(())
}

#[cfg(target_os = "windows")]
fn sync_overlay_bounds(window: &tauri::WebviewWindow) -> Result<VirtualScreenRect, String> {
    let bounds = current_virtual_screen_rect()
        .ok_or_else(|| "Virtual screen bounds not available".to_string())?;
    let hwnd = get_hwnd(window)?;

    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            bounds.left,
            bounds.top,
            bounds.width,
            bounds.height,
            SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOZORDER,
        );
    }

    Ok(bounds)
}

#[cfg(target_os = "windows")]
fn show_overlay_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    let hwnd = get_hwnd(window)?;

    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );
    }

    Ok(())
}
