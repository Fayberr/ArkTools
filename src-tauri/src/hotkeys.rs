use crate::engine::worker::now_epoch_ms;
use crate::engine::AUTOCLICKER_EXTRA_INFO;
use crate::AppHandle;
use crate::ClickerState;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;
use std::time::Instant;
use tauri::Manager;
use windows_sys::Win32::Foundation::LRESULT;
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, PeekMessageW,
    SetWindowsHookExW, UnhookWindowsHookEx, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_QUIT,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_XBUTTONDOWN, WM_XBUTTONUP,
};

const PM_REMOVE: u32 = 0x0001;
const POLL_INTERVAL: Duration = Duration::from_millis(12);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyBinding {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
    pub main_vk: i32,
    pub key_token: String,
}

pub fn register_hotkey_inner(app: &AppHandle, hotkey: String) -> Result<String, String> {
    let binding = parse_hotkey_binding(&hotkey)?;
    let state = app.state::<ClickerState>();
    state
        .suppress_hotkey_until_ms
        .store(now_epoch_ms().saturating_add(250), Ordering::SeqCst);
    state
        .suppress_hotkey_until_release
        .store(true, Ordering::SeqCst);
    Ok(format_hotkey_binding(&binding))
}

pub fn normalize_hotkey(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn parse_hotkey_binding(hotkey: &str) -> Result<HotkeyBinding, String> {
    let normalized = normalize_hotkey(hotkey);
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut super_key = false;
    let mut main_key: Option<(i32, String)> = None;

    for token in normalized.split('+').map(str::trim) {
        if token.is_empty() {
            return Err(format!("Invalid hotkey '{hotkey}': found empty key token"));
        }

        match normalize_modifier_token(token) {
            Some("ctrl") => ctrl = true,
            Some("alt") => alt = true,
            Some("shift") => shift = true,
            Some("super") => super_key = true,
            Some(_) => {}
            None => {
                if main_key
                    .replace(parse_hotkey_main_key(token, hotkey)?)
                    .is_some()
                {
                    return Err(format!(
                        "Invalid hotkey '{hotkey}': use modifiers first and only one main key"
                    ));
                }
            }
        }
    }

    let (main_vk, key_token) =
        main_key.ok_or_else(|| format!("Invalid hotkey '{hotkey}': missing main key"))?;

    Ok(HotkeyBinding {
        ctrl,
        alt,
        shift,
        super_key,
        main_vk,
        key_token,
    })
}

pub fn parse_hotkey_main_key(token: &str, original_hotkey: &str) -> Result<(i32, String), String> {
    let lower = token.trim().to_ascii_lowercase();

    if let Some(binding) = parse_named_key_token(&lower) {
        return Ok(binding);
    }

    if let Some(binding) = parse_mouse_button_token(&lower) {
        return Ok(binding);
    }

    if let Some(binding) = parse_numpad_token(&lower) {
        return Ok(binding);
    }

    if let Some(binding) = parse_function_key_token(&lower) {
        return Ok(binding);
    }

    if let Some(letter) = lower.strip_prefix("key") {
        if letter.len() == 1 {
            return parse_hotkey_main_key(letter, original_hotkey);
        }
    }

    if let Some(digit) = lower.strip_prefix("digit") {
        if digit.len() == 1 {
            return parse_hotkey_main_key(digit, original_hotkey);
        }
    }

    if lower.len() == 1 {
        let ch = lower.as_bytes()[0];
        if ch.is_ascii_lowercase() {
            return Ok((ch.to_ascii_uppercase() as i32, lower));
        }
        if ch.is_ascii_digit() {
            return Ok((ch as i32, lower));
        }
    }

    Err(format!(
        "Couldn't recognize '{token}' as a valid key in '{original_hotkey}'"
    ))
}

pub fn format_hotkey_binding(binding: &HotkeyBinding) -> String {
    let mut parts: Vec<String> = Vec::new();

    if binding.ctrl {
        parts.push(String::from("ctrl"));
    }
    if binding.alt {
        parts.push(String::from("alt"));
    }
    if binding.shift {
        parts.push(String::from("shift"));
    }
    if binding.super_key {
        parts.push(String::from("super"));
    }

    parts.push(binding.key_token.clone());
    parts.join("+")
}

static PHYSICAL_KEY_STATE: OnceLock<&'static [AtomicBool; 256]> = OnceLock::new();
static HOOKS_ACTIVE: AtomicBool = AtomicBool::new(false);

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
static LAST_DOWN_TIME: OnceLock<[std::sync::atomic::AtomicU64; 256]> = OnceLock::new();
static KEY_RELEASED: OnceLock<[std::sync::atomic::AtomicBool; 256]> = OnceLock::new();

pub static AUTO_WALK_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static AUTO_WALK_SHIFT: AtomicBool = AtomicBool::new(false);
pub static AUTO_TEK_LEGS_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static AUTO_HOLD_E_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static ANTI_AFK_ACTIVE: AtomicBool = AtomicBool::new(false);

fn physical_key_state() -> &'static [AtomicBool; 256] {
    PHYSICAL_KEY_STATE
        .get_or_init(|| Box::leak(Box::new(std::array::from_fn(|_| AtomicBool::new(false)))))
}

fn get_last_down_time() -> &'static [std::sync::atomic::AtomicU64; 256] {
    LAST_DOWN_TIME.get_or_init(|| std::array::from_fn(|_| std::sync::atomic::AtomicU64::new(0)))
}

fn get_key_released() -> &'static [std::sync::atomic::AtomicBool; 256] {
    KEY_RELEASED.get_or_init(|| std::array::from_fn(|_| std::sync::atomic::AtomicBool::new(true)))
}

pub fn stop_auto_walk() {
    if AUTO_WALK_ACTIVE.swap(false, Ordering::SeqCst) {
        log::info!("[Macro] Stopping Auto Walk");
        crate::telemetry::track_event("macro_stop", Some("auto_walk".to_string()));
        crate::engine::keyboard::send_key_event(0x57, KEYEVENTF_KEYUP); // W keyup
        if AUTO_WALK_SHIFT.load(Ordering::SeqCst) {
            crate::engine::keyboard::send_key_event(VK_LSHIFT as u16, KEYEVENTF_KEYUP);
            // Shift keyup
        }
    }
}

pub fn stop_auto_tek_legs() {
    if AUTO_TEK_LEGS_ACTIVE.swap(false, Ordering::SeqCst) {
        log::info!("[Macro] Stopping Auto Tek Legs");
        crate::telemetry::track_event("macro_stop", Some("auto_tek_legs".to_string()));
        crate::engine::keyboard::send_key_event(VK_LCONTROL as u16, KEYEVENTF_KEYUP); // Ctrl keyup
        crate::engine::keyboard::send_key_event(VK_LSHIFT as u16, KEYEVENTF_KEYUP);
        // Shift keyup
    }
}

pub fn stop_hold_e() {
    if AUTO_HOLD_E_ACTIVE.swap(false, Ordering::SeqCst) {
        log::info!("[Macro] Stopping Hold E");
        crate::engine::keyboard::send_key_event(0x45, KEYEVENTF_KEYUP); // E keyup
    }
}

fn is_physical_vk_down(vk: i32) -> bool {
    if !(0..256).contains(&vk) {
        return false;
    }
    physical_key_state()[vk as usize].load(Ordering::Relaxed)
}

unsafe extern "system" fn mouse_ll_proc(n_code: i32, w_param: usize, l_param: isize) -> LRESULT {
    if n_code >= 0 {
        let mhs = &*(l_param as *const MSLLHOOKSTRUCT);
        if (mhs.dwExtraInfo) != AUTOCLICKER_EXTRA_INFO {
            let (vk, down) = match w_param as u32 {
                WM_LBUTTONDOWN => (VK_LBUTTON as i32, true),
                WM_LBUTTONUP => (VK_LBUTTON as i32, false),
                WM_RBUTTONDOWN => (VK_RBUTTON as i32, true),
                WM_RBUTTONUP => (VK_RBUTTON as i32, false),
                WM_MBUTTONDOWN => (VK_MBUTTON as i32, true),
                WM_MBUTTONUP => (VK_MBUTTON as i32, false),
                WM_XBUTTONDOWN => {
                    let x = if ((mhs.mouseData >> 16) & 0xFFFF) == 1 {
                        VK_XBUTTON1 as i32
                    } else {
                        VK_XBUTTON2 as i32
                    };
                    (x, true)
                }
                WM_XBUTTONUP => {
                    let x = if ((mhs.mouseData >> 16) & 0xFFFF) == 1 {
                        VK_XBUTTON1 as i32
                    } else {
                        VK_XBUTTON2 as i32
                    };
                    (x, false)
                }
                _ => (-1, false),
            };
            if vk >= 0 && (vk as usize) < 256 {
                physical_key_state()[vk as usize].store(down, Ordering::Relaxed);
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

unsafe extern "system" fn keyboard_ll_proc(n_code: i32, w_param: usize, l_param: isize) -> LRESULT {
    if n_code >= 0 {
        let khs = &*(l_param as *const KBDLLHOOKSTRUCT);
        let vk = khs.vkCode as i32;
        let down = matches!(w_param as u32, WM_KEYDOWN | WM_SYSKEYDOWN);

        if khs.dwExtraInfo != AUTOCLICKER_EXTRA_INFO {
            if (0..256).contains(&vk) {
                physical_key_state()[vk as usize].store(down, Ordering::Relaxed);

                if down {
                    // Physical press cancels toggles
                    if vk == 0x57 || vk == 0x53 || vk == 0x41 || vk == 0x44 {
                        // W, S, A, D
                        if AUTO_WALK_ACTIVE.load(Ordering::SeqCst) {
                            stop_auto_walk();
                        }
                    }
                    if vk == 0x45 {
                        // E
                        if AUTO_HOLD_E_ACTIVE.load(Ordering::SeqCst) {
                            stop_hold_e();
                        }
                    }
                    if vk == VK_LCONTROL as i32
                        || vk == VK_RCONTROL as i32
                        || vk == VK_LSHIFT as i32
                        || vk == VK_RSHIFT as i32
                    {
                        if AUTO_TEK_LEGS_ACTIVE.load(Ordering::SeqCst) {
                            stop_auto_tek_legs();
                        }
                    }

                    let released = get_key_released()[vk as usize].swap(false, Ordering::SeqCst);
                    if released {
                        let now = now_epoch_ms();
                        let prev = get_last_down_time()[vk as usize].swap(now, Ordering::SeqCst);
                        let diff = now.saturating_sub(prev);
                        if diff < 250 {
                            let app_opt = APP_HANDLE.get();
                            let mut target_match = true;
                            if let Some(app) = app_opt {
                                let state = app.state::<ClickerState>();
                                let settings = state.settings.lock().unwrap();
                                target_match = is_foreground_window_matching(
                                    &settings.target_process,
                                    &settings.target_window,
                                );
                            }

                            if target_match {
                                if vk == 0x57 {
                                    // W
                                    if let Some(app) = app_opt {
                                        let state = app.state::<ClickerState>();
                                        let settings = state.settings.lock().unwrap();
                                        if let Some(m) =
                                            settings.macros.iter().find(|m| m.id == "auto_walk")
                                        {
                                            if m.enabled {
                                                if !AUTO_WALK_ACTIVE.load(Ordering::SeqCst) {
                                                    log::info!("[Macro] Double-tap detected: Starting Auto Walk");
                                                    crate::telemetry::track_event(
                                                        "macro_start",
                                                        Some("auto_walk".to_string()),
                                                    );
                                                    AUTO_WALK_SHIFT
                                                        .store(m.hold_sprint, Ordering::SeqCst);
                                                    AUTO_WALK_ACTIVE.store(true, Ordering::SeqCst);
                                                    crate::engine::keyboard::send_key_event(
                                                        0x57, 0,
                                                    );
                                                    if m.hold_sprint {
                                                        crate::engine::keyboard::send_key_event(
                                                            VK_LSHIFT as u16,
                                                            0,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                if vk == VK_LCONTROL as i32 || vk == VK_CONTROL as i32 {
                                    if let Some(app) = app_opt {
                                        let state = app.state::<ClickerState>();
                                        let settings = state.settings.lock().unwrap();
                                        if let Some(m) =
                                            settings.macros.iter().find(|m| m.id == "auto_tek_legs")
                                        {
                                            if m.enabled {
                                                if !AUTO_TEK_LEGS_ACTIVE.load(Ordering::SeqCst) {
                                                    log::info!("[Macro] Double-tap detected: Starting Auto Tek Legs");
                                                    crate::telemetry::track_event(
                                                        "macro_start",
                                                        Some("auto_tek_legs".to_string()),
                                                    );
                                                    AUTO_TEK_LEGS_ACTIVE
                                                        .store(true, Ordering::SeqCst);
                                                    crate::engine::keyboard::send_key_event(
                                                        VK_LCONTROL as u16,
                                                        0,
                                                    );
                                                    crate::engine::keyboard::send_key_event(
                                                        VK_LSHIFT as u16,
                                                        0,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    get_key_released()[vk as usize].store(true, Ordering::SeqCst);
                }
            }
        }

        if !down {
            if vk == 0x57 && AUTO_WALK_ACTIVE.load(Ordering::SeqCst) {
                return 1; // Block key release
            }
            if vk == 0x45 && AUTO_HOLD_E_ACTIVE.load(Ordering::SeqCst) {
                return 1; // Block key release
            }
            if (vk == VK_LCONTROL as i32
                || vk == VK_CONTROL as i32
                || vk == VK_LSHIFT as i32
                || vk == VK_SHIFT as i32)
                && AUTO_TEK_LEGS_ACTIVE.load(Ordering::SeqCst)
            {
                return 1; // Block key release
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

pub fn get_process_name_by_pid(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }

        let mut buffer = [0u16; 1024];
        let mut size = buffer.len() as u32;
        let success = QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size);
        windows_sys::Win32::Foundation::CloseHandle(handle);

        if success != 0 && size > 0 {
            let full_path = String::from_utf16_lossy(&buffer[..size as usize]);
            if let Some(filename) = std::path::Path::new(&full_path)
                .file_name()
                .and_then(|n| n.to_str())
            {
                return Some(filename.to_string());
            }
        }
        None
    }
}

fn is_foreground_window_matching(target_process: &str, target_title: &str) -> bool {
    if target_process.trim().is_empty() && target_title.trim().is_empty() {
        return true;
    }
    unsafe {
        let hwnd = GetForegroundWindow();
        let val = hwnd as isize;
        if val == 0 {
            return false;
        }

        // 1. Check process name if specified
        if !target_process.trim().is_empty() {
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid > 0 {
                if let Some(proc_name) = get_process_name_by_pid(pid) {
                    if proc_name.to_lowercase() == target_process.to_lowercase() {
                        return true;
                    }
                }
            }
            return false;
        }

        // 2. Check window title if specified
        if !target_title.trim().is_empty() {
            let mut buffer = [0u16; 512];
            let len = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
            if len > 0 {
                let title = String::from_utf16_lossy(&buffer[..len as usize]);
                let title_lower = title.to_lowercase();
                let target_lower = target_title.to_lowercase();
                if title_lower.contains(&target_lower) {
                    return true;
                }
            }
            return false;
        }
    }
    false
}

pub fn start_hotkey_listener(app: AppHandle) {
    let _ = APP_HANDLE.set(app.clone());

    // Spawn repeat key daemon thread to keep keys locked down
    std::thread::spawn(|| {
        loop {
            std::thread::sleep(Duration::from_millis(85));
            if AUTO_WALK_ACTIVE.load(Ordering::SeqCst) {
                crate::engine::keyboard::send_key_event(0x57, 0); // VK_W
                if AUTO_WALK_SHIFT.load(Ordering::SeqCst) {
                    crate::engine::keyboard::send_key_event(VK_LSHIFT as u16, 0);
                }
            }
            if AUTO_HOLD_E_ACTIVE.load(Ordering::SeqCst) {
                crate::engine::keyboard::send_key_event(0x45, 0); // VK_E
            }
            if AUTO_TEK_LEGS_ACTIVE.load(Ordering::SeqCst) {
                crate::engine::keyboard::send_key_event(VK_LCONTROL as u16, 0);
                crate::engine::keyboard::send_key_event(VK_LSHIFT as u16, 0);
            }
        }
    });

    // Spawn Anti-AFK daemon thread
    std::thread::spawn(|| {
        let mut last_action_time = Instant::now();
        let mut next_trigger_secs = 12 + get_pseudo_random(6); // 12-18 seconds default initial

        fn get_pseudo_random(max: u32) -> u32 {
            let time = now_epoch_ms();
            let seed = (time ^ (time >> 12)) as u32;
            let next = seed.wrapping_mul(1103515245).wrapping_add(12345);
            next % max
        }

        fn run_anti_afk_step() {
            // Phase 1: Camera rotation (Relative movement)
            // Rotate camera randomly, perform movement, then rotate it back.
            let rotate_x = (150 + get_pseudo_random(200)) as i32;
            let rotate_y = (50 + get_pseudo_random(100)) as i32;

            let dir_x = if get_pseudo_random(2) == 0 { 1 } else { -1 };
            let dir_y = if get_pseudo_random(2) == 0 { 1 } else { -1 };

            let final_dx = rotate_x * dir_x;
            let final_dy = rotate_y * dir_y;

            log::info!(
                "[Anti-AFK] Rotating camera (dx: {}, dy: {})",
                final_dx,
                final_dy
            );
            crate::engine::mouse::send_mouse_move_relative(final_dx, final_dy);
            std::thread::sleep(Duration::from_millis(200 + get_pseudo_random(200) as u64));

            // Choose a complex action sequence
            let action = get_pseudo_random(3);
            match action {
                0 => {
                    log::info!("[Anti-AFK] Walk W/S sequence");
                    // Walk forward
                    crate::engine::keyboard::send_key_event(0x57, 0); // W down
                    let walk_time = 600 + get_pseudo_random(400) as u64;

                    // 30% chance to jump while walking
                    if get_pseudo_random(10) < 3 {
                        std::thread::sleep(Duration::from_millis(walk_time / 2));
                        crate::engine::keyboard::send_key_event(VK_SPACE as u16, 0); // Space down
                        std::thread::sleep(Duration::from_millis(50));
                        crate::engine::keyboard::send_key_event(VK_SPACE as u16, KEYEVENTF_KEYUP); // Space up
                        std::thread::sleep(Duration::from_millis(walk_time / 2));
                    } else {
                        std::thread::sleep(Duration::from_millis(walk_time));
                    }
                    crate::engine::keyboard::send_key_event(0x57, KEYEVENTF_KEYUP); // W up

                    std::thread::sleep(Duration::from_millis(200 + get_pseudo_random(100) as u64));

                    // Attack/Punch
                    log::info!("[Anti-AFK] Punching");
                    crate::engine::mouse::send_mouse_event(
                        windows_sys::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
                    );
                    std::thread::sleep(Duration::from_millis(50));
                    crate::engine::mouse::send_mouse_event(
                        windows_sys::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTUP,
                    );

                    std::thread::sleep(Duration::from_millis(200 + get_pseudo_random(100) as u64));

                    // Walk backward to return to position
                    crate::engine::keyboard::send_key_event(0x53, 0); // S down
                    std::thread::sleep(Duration::from_millis(walk_time + 50));
                    crate::engine::keyboard::send_key_event(0x53, KEYEVENTF_KEYUP);
                    // S up
                }
                1 => {
                    log::info!("[Anti-AFK] Sideways A/D sequence");
                    // Walk left
                    crate::engine::keyboard::send_key_event(0x41, 0); // A down
                    let walk_time = 600 + get_pseudo_random(400) as u64;
                    std::thread::sleep(Duration::from_millis(walk_time));
                    crate::engine::keyboard::send_key_event(0x41, KEYEVENTF_KEYUP); // A up

                    std::thread::sleep(Duration::from_millis(200 + get_pseudo_random(100) as u64));

                    // Jump
                    crate::engine::keyboard::send_key_event(VK_SPACE as u16, 0); // Space down
                    std::thread::sleep(Duration::from_millis(50));
                    crate::engine::keyboard::send_key_event(VK_SPACE as u16, KEYEVENTF_KEYUP); // Space up

                    std::thread::sleep(Duration::from_millis(200 + get_pseudo_random(100) as u64));

                    // Walk right to return to position
                    crate::engine::keyboard::send_key_event(0x44, 0); // D down
                    std::thread::sleep(Duration::from_millis(walk_time + 50));
                    crate::engine::keyboard::send_key_event(0x44, KEYEVENTF_KEYUP);
                    // D up
                }
                _ => {
                    log::info!("[Anti-AFK] Jump & Punch sequence");
                    crate::engine::keyboard::send_key_event(VK_SPACE as u16, 0); // Space down
                    std::thread::sleep(Duration::from_millis(50));
                    crate::engine::keyboard::send_key_event(VK_SPACE as u16, KEYEVENTF_KEYUP); // Space up

                    std::thread::sleep(Duration::from_millis(200 + get_pseudo_random(100) as u64));

                    crate::engine::mouse::send_mouse_event(
                        windows_sys::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
                    );
                    std::thread::sleep(Duration::from_millis(50));
                    crate::engine::mouse::send_mouse_event(
                        windows_sys::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_LEFTUP,
                    );
                }
            }

            std::thread::sleep(Duration::from_millis(200 + get_pseudo_random(200) as u64));

            // Rotate camera back to original position
            log::info!(
                "[Anti-AFK] Rotating camera back (dx: {}, dy: {})",
                -final_dx,
                -final_dy
            );
            crate::engine::mouse::send_mouse_move_relative(-final_dx, -final_dy);
        }

        loop {
            std::thread::sleep(Duration::from_millis(500));
            if ANTI_AFK_ACTIVE.load(Ordering::SeqCst) {
                if last_action_time.elapsed() >= Duration::from_secs(next_trigger_secs as u64) {
                    last_action_time = Instant::now();
                    next_trigger_secs = 12 + get_pseudo_random(7); // Randomize next run between 12 and 18 seconds

                    let app_opt = APP_HANDLE.get();
                    let mut target_match = true;
                    if let Some(app) = app_opt {
                        let state = app.state::<ClickerState>();
                        let settings = state.settings.lock().unwrap();
                        target_match = is_foreground_window_matching(
                            &settings.target_process,
                            &settings.target_window,
                        );
                    }

                    if target_match {
                        run_anti_afk_step();
                    }
                }
            } else {
                last_action_time = Instant::now();
                next_trigger_secs = 12 + get_pseudo_random(7);
            }
        }
    });

    std::thread::spawn(move || unsafe {
        let mouse_hook =
            SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_ll_proc), std::ptr::null_mut(), 0);
        let kb_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(keyboard_ll_proc),
            std::ptr::null_mut(),
            0,
        );

        if !mouse_hook.is_null() && !kb_hook.is_null() {
            HOOKS_ACTIVE.store(true, Ordering::SeqCst);
        }

        let mut pressed_states: std::collections::HashMap<String, bool> =
            std::collections::HashMap::new();
        let mut last_trigger_times: std::collections::HashMap<String, u64> =
            std::collections::HashMap::new();
        let mut last_check = Instant::now();
        let mut msg: MSG = std::mem::zeroed();

        loop {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    if !mouse_hook.is_null() {
                        UnhookWindowsHookEx(mouse_hook);
                    }
                    if !kb_hook.is_null() {
                        UnhookWindowsHookEx(kb_hook);
                    }
                    return;
                }
            }

            if last_check.elapsed() >= POLL_INTERVAL {
                last_check = Instant::now();

                let (hotkeys, strict) = {
                    let state = app.state::<ClickerState>();
                    let hotkeys = state.registered_hotkeys.lock().unwrap().clone();
                    let strict = false;
                    (hotkeys, strict)
                };

                let suppress_until = app
                    .state::<ClickerState>()
                    .suppress_hotkey_until_ms
                    .load(Ordering::SeqCst);
                let suppress_until_release = app
                    .state::<ClickerState>()
                    .suppress_hotkey_until_release
                    .load(Ordering::SeqCst);
                let hotkey_capture_active = app
                    .state::<ClickerState>()
                    .hotkey_capture_active
                    .load(Ordering::SeqCst);
                let sequence_pick_active = app
                    .state::<ClickerState>()
                    .sequence_pick_active
                    .load(Ordering::SeqCst);
                let custom_stop_zone_pick_active = app
                    .state::<ClickerState>()
                    .custom_stop_zone_pick_active
                    .load(Ordering::SeqCst);

                if hotkey_capture_active || sequence_pick_active || custom_stop_zone_pick_active {
                    continue;
                }

                if suppress_until_release {
                    let mut any_pressed = false;
                    for (_, binding) in &hotkeys {
                        let pressed = if HOOKS_ACTIVE.load(Ordering::Relaxed) {
                            is_hotkey_binding_pressed_physical(binding, strict)
                        } else {
                            is_hotkey_binding_pressed(binding, strict)
                        };
                        if pressed {
                            any_pressed = true;
                        }
                    }
                    if !any_pressed {
                        app.state::<ClickerState>()
                            .suppress_hotkey_until_release
                            .store(false, Ordering::SeqCst);
                    }
                    continue;
                }

                if now_epoch_ms() < suppress_until {
                    continue;
                }

                for (macro_id, binding) in hotkeys {
                    if macro_id == "auto_walk" || macro_id == "auto_tek_legs" {
                        continue;
                    }
                    let currently_pressed = if HOOKS_ACTIVE.load(Ordering::Relaxed) {
                        is_hotkey_binding_pressed_physical(&binding, strict)
                    } else {
                        is_hotkey_binding_pressed(&binding, strict)
                    };

                    let was_pressed = pressed_states.entry(macro_id.clone()).or_insert(false);

                    if currently_pressed && !*was_pressed {
                        let now = now_epoch_ms();
                        let last_trigger = last_trigger_times.entry(macro_id.clone()).or_insert(0);
                        if now.saturating_sub(*last_trigger) >= 400 {
                            *last_trigger = now;

                            let (target_process, target_window) = {
                                let state = app.state::<ClickerState>();
                                let settings = state.settings.lock().unwrap();
                                (
                                    settings.target_process.clone(),
                                    settings.target_window.clone(),
                                )
                            };

                            if is_foreground_window_matching(&target_process, &target_window) {
                                log::info!("[Hotkeys] Hotkey triggered for macro: {}", macro_id);
                                let state = app.state::<ClickerState>();
                                state
                                    .suppress_hotkey_until_ms
                                    .store(now.saturating_add(400), Ordering::SeqCst);
                                state
                                    .suppress_hotkey_until_release
                                    .store(true, Ordering::SeqCst);

                                let app_clone = app.clone();
                                let macro_id_clone = macro_id.clone();
                                std::thread::spawn(move || {
                                    if let Err(e) =
                                        crate::ui_commands::run_macro(app_clone, macro_id_clone)
                                    {
                                        log::error!("[Macro] Failed to run macro: {}", e);
                                    }
                                });
                            } else {
                                log::info!(
                                    "[Hotkeys] Hotkey pressed but foreground window does not match process='{}' title='{}'",
                                    target_process, target_window
                                );
                            }
                        }
                    }

                    *was_pressed = currently_pressed;
                }
            } else {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    });
}

fn is_hotkey_binding_pressed_physical(binding: &HotkeyBinding, strict: bool) -> bool {
    let ctrl_down =
        is_physical_vk_down(VK_LCONTROL as i32) || is_physical_vk_down(VK_RCONTROL as i32);
    let alt_down = is_physical_vk_down(VK_LMENU as i32) || is_physical_vk_down(VK_RMENU as i32);
    let shift_down = is_physical_vk_down(VK_LSHIFT as i32) || is_physical_vk_down(VK_RSHIFT as i32);
    let super_down = is_physical_vk_down(VK_LWIN as i32) || is_physical_vk_down(VK_RWIN as i32);
    if !modifiers_match(binding, ctrl_down, alt_down, shift_down, super_down, strict) {
        return false;
    }
    is_physical_vk_down(binding.main_vk)
}

pub fn is_hotkey_binding_pressed(binding: &HotkeyBinding, strict: bool) -> bool {
    let ctrl_down = is_vk_down(VK_CONTROL as i32);
    let alt_down = is_vk_down(VK_MENU as i32);
    let shift_down = is_vk_down(VK_SHIFT as i32);
    let super_down = is_vk_down(VK_LWIN as i32) || is_vk_down(VK_RWIN as i32);

    if !modifiers_match(binding, ctrl_down, alt_down, shift_down, super_down, strict) {
        return false;
    }

    is_vk_down(binding.main_vk)
}

fn modifiers_match(
    binding: &HotkeyBinding,
    ctrl_down: bool,
    alt_down: bool,
    shift_down: bool,
    super_down: bool,
    strict: bool,
) -> bool {
    if binding.ctrl && !ctrl_down {
        return false;
    }
    if binding.alt && !alt_down {
        return false;
    }
    if binding.shift && !shift_down {
        return false;
    }
    if binding.super_key && !super_down {
        return false;
    }

    if strict {
        if ctrl_down && !binding.ctrl {
            return false;
        }
        if alt_down && !binding.alt {
            return false;
        }
        if shift_down && !binding.shift {
            return false;
        }
        if super_down && !binding.super_key {
            return false;
        }
    }

    true
}

pub fn is_vk_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

fn normalize_modifier_token(token: &str) -> Option<&'static str> {
    match token {
        "alt" | "option" => Some("alt"),
        "ctrl" | "control" => Some("ctrl"),
        "shift" => Some("shift"),
        "super" | "command" | "cmd" | "meta" | "win" => Some("super"),
        _ => None,
    }
}

fn binding(vk: i32, token: &str) -> (i32, String) {
    (vk, token.to_string())
}

fn parse_named_key_token(token: &str) -> Option<(i32, String)> {
    match token {
        "<" | ">" | "intlbackslash" | "oem102" | "nonusbackslash" => {
            Some(binding(VK_OEM_102 as i32, "IntlBackslash"))
        }
        "space" | "spacebar" => Some(binding(VK_SPACE as i32, "space")),
        "tab" => Some(binding(VK_TAB as i32, "tab")),
        "enter" | "return" => Some(binding(VK_RETURN as i32, "enter")),
        "backspace" => Some(binding(VK_BACK as i32, "backspace")),
        "delete" | "del" => Some(binding(VK_DELETE as i32, "delete")),
        "insert" | "ins" => Some(binding(VK_INSERT as i32, "insert")),
        "home" => Some(binding(VK_HOME as i32, "home")),
        "end" => Some(binding(VK_END as i32, "end")),
        "pageup" | "pgup" => Some(binding(VK_PRIOR as i32, "pageup")),
        "pagedown" | "pgdn" => Some(binding(VK_NEXT as i32, "pagedown")),
        "up" | "arrowup" => Some(binding(VK_UP as i32, "up")),
        "down" | "arrowdown" => Some(binding(VK_DOWN as i32, "down")),
        "left" | "arrowleft" => Some(binding(VK_LEFT as i32, "left")),
        "right" | "arrowright" => Some(binding(VK_RIGHT as i32, "right")),
        "esc" | "escape" => Some(binding(VK_ESCAPE as i32, "escape")),
        "capslock" => Some(binding(VK_CAPITAL as i32, "capslock")),
        "numlock" => Some(binding(VK_NUMLOCK as i32, "numlock")),
        "scrolllock" => Some(binding(VK_SCROLL as i32, "scrolllock")),
        "menu" | "apps" | "contextmenu" => Some(binding(VK_APPS as i32, "menu")),
        "printscreen" | "prtsc" | "snapshot" => Some(binding(VK_SNAPSHOT as i32, "printscreen")),
        "pause" | "break" => Some(binding(VK_PAUSE as i32, "pause")),
        "/" | "slash" => Some(binding(VK_OEM_2 as i32, "/")),
        "\\" | "backslash" => Some(binding(VK_OEM_5 as i32, "\\")),
        ";" | "semicolon" => Some(binding(VK_OEM_1 as i32, ";")),
        "'" | "quote" | "apostrophe" => Some(binding(VK_OEM_7 as i32, "'")),
        "[" | "bracketleft" => Some(binding(VK_OEM_4 as i32, "[")),
        "]" | "bracketright" => Some(binding(VK_OEM_6 as i32, "]")),
        "-" | "minus" => Some(binding(VK_OEM_MINUS as i32, "-")),
        "=" | "equal" => Some(binding(VK_OEM_PLUS as i32, "=")),
        "`" | "backquote" | "grave" => Some(binding(VK_OEM_3 as i32, "`")),
        "," | "comma" => Some(binding(VK_OEM_COMMA as i32, ",")),
        "." | "period" | "dot" => Some(binding(VK_OEM_PERIOD as i32, ".")),
        _ => None,
    }
}

fn parse_mouse_button_token(token: &str) -> Option<(i32, String)> {
    match token {
        "mouseleft" | "leftmouse" | "leftbutton" | "mouse1" | "lmb" => {
            Some(binding(VK_LBUTTON as i32, "mouseleft"))
        }
        "mouseright" | "rightmouse" | "rightbutton" | "mouse2" | "rmb" => {
            Some(binding(VK_RBUTTON as i32, "mouseright"))
        }
        "mousemiddle" | "middlemouse" | "middlebutton" | "mouse3" | "mmb" | "scrollbutton"
        | "middleclick" => Some(binding(VK_MBUTTON as i32, "mousemiddle")),
        "mouse4" | "xbutton1" | "mouseback" | "browserback" | "backbutton" => {
            Some(binding(VK_XBUTTON1 as i32, "mouse4"))
        }
        "mouse5" | "xbutton2" | "mouseforward" | "browserforward" | "forwardbutton" => {
            Some(binding(VK_XBUTTON2 as i32, "mouse5"))
        }
        _ => None,
    }
}

fn parse_numpad_token(token: &str) -> Option<(i32, String)> {
    match token {
        "numpad0" | "num0" => Some(binding(VK_NUMPAD0 as i32, "numpad0")),
        "numpad1" | "num1" => Some(binding(VK_NUMPAD1 as i32, "numpad1")),
        "numpad2" | "num2" => Some(binding(VK_NUMPAD2 as i32, "numpad2")),
        "numpad3" | "num3" => Some(binding(VK_NUMPAD3 as i32, "numpad3")),
        "numpad4" | "num4" => Some(binding(VK_NUMPAD4 as i32, "numpad4")),
        "numpad5" | "num5" => Some(binding(VK_NUMPAD5 as i32, "numpad5")),
        "numpad6" | "num6" => Some(binding(VK_NUMPAD6 as i32, "numpad6")),
        "numpad7" | "num7" => Some(binding(VK_NUMPAD7 as i32, "numpad7")),
        "numpad8" | "num8" => Some(binding(VK_NUMPAD8 as i32, "numpad8")),
        "numpad9" | "num9" => Some(binding(VK_NUMPAD9 as i32, "numpad9")),
        "numpadadd" | "numadd" | "numpadplus" | "numplus" => {
            Some(binding(VK_ADD as i32, "numpadadd"))
        }
        "numpadsubtract" | "numsubtract" | "numsub" | "numpadminus" | "numminus" => {
            Some(binding(VK_SUBTRACT as i32, "numpadsubtract"))
        }
        "numpadmultiply" | "nummultiply" | "nummul" | "numpadmul" => {
            Some(binding(VK_MULTIPLY as i32, "numpadmultiply"))
        }
        "numpaddivide" | "numdivide" | "numdiv" | "numpaddiv" => {
            Some(binding(VK_DIVIDE as i32, "numpaddivide"))
        }
        "numpaddecimal" | "numdecimal" | "numdot" | "numdel" | "numpadpoint" => {
            Some(binding(VK_DECIMAL as i32, "numpaddecimal"))
        }
        _ => None,
    }
}

fn parse_function_key_token(token: &str) -> Option<(i32, String)> {
    if !token.starts_with('f') || token.len() > 3 {
        return None;
    }

    let number = token[1..].parse::<i32>().ok()?;
    let vk = match number {
        1..=24 => VK_F1 as i32 + (number - 1),
        _ => return None,
    };

    Some(binding(vk, token))
}

#[cfg(test)]
mod tests {
    use super::{format_hotkey_binding, modifiers_match, parse_hotkey_binding};

    #[test]
    fn numpad_tokens_round_trip() {
        for token in [
            "numpad0",
            "numpad1",
            "numpad2",
            "numpad3",
            "numpad4",
            "numpad5",
            "numpad6",
            "numpad7",
            "numpad8",
            "numpad9",
            "numpadadd",
            "numpadsubtract",
            "numpadmultiply",
            "numpaddivide",
            "numpaddecimal",
        ] {
            let hotkey = format!("ctrl+shift+{token}");
            let binding = parse_hotkey_binding(&hotkey).expect("token should parse");
            assert_eq!(binding.key_token, token);
            assert_eq!(format_hotkey_binding(&binding), hotkey);
        }
    }

    #[test]
    fn empty_hotkeys_are_rejected() {
        assert!(parse_hotkey_binding("").is_err());
        assert!(parse_hotkey_binding("ctrl+").is_err());
    }

    #[test]
    fn extra_modifiers_do_not_block_hotkeys_in_relaxed_mode() {
        let binding = parse_hotkey_binding("f11").expect("hotkey should parse");
        assert!(modifiers_match(&binding, false, false, true, false, false));
        assert!(modifiers_match(&binding, true, true, true, true, false));
    }

    #[test]
    fn extra_modifiers_block_hotkeys_in_strict_mode() {
        let binding = parse_hotkey_binding("f11").expect("hotkey should parse");
        assert!(!modifiers_match(&binding, false, false, true, false, true));
        assert!(!modifiers_match(&binding, true, true, true, true, true));
        assert!(modifiers_match(&binding, false, false, false, false, true));
    }
}
