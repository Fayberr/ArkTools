mod settings;
use settings::ClickerSettings;
mod app_state;
mod autostart;
mod engine;
mod hotkeys;
mod overlay;
mod sequence_picker;
mod telemetry;
mod ui_commands;

use crate::app_state::ClickerState;
use crate::engine::worker::emit_status;
use crate::hotkeys::start_hotkey_listener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

const STATUS_EVENT: &str = "clicker-status";
use raw_window_handle::HasWindowHandle;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ClickerState {
            running: Arc::new(AtomicBool::new(false)),
            run_generation: std::sync::atomic::AtomicU64::new(0),
            settings: Mutex::new(ClickerSettings::default()),
            last_error: Mutex::new(None),
            stop_reason: Mutex::new(None),
            active_sequence_index: std::sync::atomic::AtomicI64::new(-1),
            active_sequence_tick: std::sync::atomic::AtomicU64::new(0),
            registered_hotkeys: Mutex::new(Vec::new()),
            suppress_hotkey_until_ms: std::sync::atomic::AtomicU64::new(0),
            suppress_hotkey_until_release: AtomicBool::new(false),
            hotkey_capture_active: AtomicBool::new(false),
            sequence_pick_active: AtomicBool::new(false),
            custom_stop_zone_pick_active: AtomicBool::new(false),
            settings_initialized: AtomicBool::new(false),
        })
        .setup(|app| {
            if cfg!(debug_assertions) {
                let _ = app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                );
            }

            let show_item = MenuItem::with_id(app, "show", "Show", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("ArkTools")
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        crate::overlay::OVERLAY_THREAD_RUNNING
                            .store(false, std::sync::atomic::Ordering::SeqCst);
                        crate::sequence_picker::cancel_sequence_point_pick_inner(app);
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Initialize registered hotkeys from default/saved settings
            let state = app.state::<ClickerState>();
            let mut initial = Vec::new();
            for m in &state.settings.lock().unwrap().macros {
                if m.enabled && !m.hotkey.is_empty() {
                    if let Ok(binding) = crate::hotkeys::parse_hotkey_binding(&m.hotkey) {
                        initial.push((m.id.clone(), binding));
                    }
                }
            }
            *state.registered_hotkeys.lock().unwrap() = initial;

            let handle = app.handle().clone();
            start_hotkey_listener(handle.clone());
            emit_status(&handle);
            crate::telemetry::track_event("app_boot", None);

            let handle_clone = handle.clone();
            std::thread::spawn(move || {
                let mut retry_count = 0;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(30));
                    if let Some(window) = handle_clone.get_webview_window("overlay") {
                        if window.window_handle().is_ok() {
                            if let Err(e) = overlay::init_overlay(&handle_clone) {
                                log::error!("[Overlay] Failed to initialize overlay: {}", e);
                            }
                            break;
                        }
                    }
                    retry_count += 1;
                    if retry_count > 100 {
                        log::error!("[Overlay] Failed to get overlay window handle after timeout");
                        break;
                    }
                }
            });

            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }

            if std::env::args().any(|a| a == "--autostart") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ui_commands::set_webview_zoom,
            ui_commands::get_text_scale_factor,
            ui_commands::update_settings,
            ui_commands::get_settings,
            ui_commands::reset_settings,
            ui_commands::get_status,
            ui_commands::register_hotkey,
            ui_commands::set_hotkey_capture_active,
            ui_commands::pick_position,
            ui_commands::start_sequence_point_pick,
            ui_commands::cancel_sequence_point_pick,
            ui_commands::get_app_info,
            overlay::hide_overlay,
            ui_commands::quit_app,
            ui_commands::get_autostart_enabled,
            ui_commands::set_autostart_enabled,
            ui_commands::run_macro,
            ui_commands::get_open_windows,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                label,
                ..
            } = &event
            {
                if label == "main" {
                    api.prevent_close();
                    crate::overlay::OVERLAY_THREAD_RUNNING
                        .store(false, std::sync::atomic::Ordering::SeqCst);
                    crate::sequence_picker::cancel_sequence_point_pick_inner(app_handle);
                    app_handle.exit(0);
                }
            }
        });
}
