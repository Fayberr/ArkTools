use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_SCANCODE, MAPVK_VK_TO_VSC_EX,
};

use super::AUTOCLICKER_EXTRA_INFO;

#[inline]
fn vk_to_scan(vk: u16) -> (u16, bool) {
    let raw = unsafe { MapVirtualKeyW(vk as u32, MAPVK_VK_TO_VSC_EX) };
    ((raw & 0xFF) as u16, (raw >> 8) != 0)
}

#[inline]
pub fn make_keyboard_input(vk: u16, flags: u32) -> INPUT {
    let (scan, extended) = vk_to_scan(vk);
    let ext_flag = if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows_sys::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: scan,
                dwFlags: flags | KEYEVENTF_SCANCODE | ext_flag,
                time: 0,
                dwExtraInfo: AUTOCLICKER_EXTRA_INFO,
            },
        },
    }
}

#[inline]
pub fn send_key_event(vk: u16, flags: u32) {
    let input = make_keyboard_input(vk, flags);
    unsafe { SendInput(1, &input, std::mem::size_of::<INPUT>() as i32) };
}
