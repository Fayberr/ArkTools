use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

static TELEMETRY_ENABLED: AtomicBool = AtomicBool::new(true);

#[derive(Serialize)]
struct TelemetryPayload {
    client_version: String,
    event_name: String,
    timestamp: u64,
    details: Option<String>,
}

pub fn track_event(event_name: &str, details: Option<String>) {
    if !TELEMETRY_ENABLED.load(Ordering::Relaxed) {
        return;
    }

    let payload = TelemetryPayload {
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        event_name: event_name.to_string(),
        timestamp: crate::engine::worker::now_epoch_ms(),
        details,
    };

    std::thread::spawn(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_millis(1500)) // 1.5s timeout
            .build();

        if let Ok(client) = client {
            let _ = client
                .post("https://arktools.fayber.dev/api/telemetry")
                .json(&payload)
                .send();
        }
    });
}
