use crate::app_state::ClickerStatusPayload;
use crate::ClickerState;
use crate::STATUS_EVENT;
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

pub fn current_status(app: &AppHandle) -> ClickerStatusPayload {
    let state = app.state::<ClickerState>();
    let last_error = state.last_error.lock().unwrap().clone();
    let stop_reason = state.stop_reason.lock().unwrap().clone();
    let active_sequence_index = state.active_sequence_index.load(Ordering::SeqCst);
    let active_sequence_tick = state.active_sequence_tick.load(Ordering::SeqCst);

    ClickerStatusPayload {
        running: false,
        click_count: 0,
        last_error,
        stop_reason,
        active_sequence_index: if active_sequence_index >= 0 {
            Some(active_sequence_index as usize)
        } else {
            None
        },
        active_sequence_tick,
    }
}

pub fn emit_status(app: &AppHandle) {
    let _ = app.emit(STATUS_EVENT, current_status(app));
}

pub fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
