//! Hyprland IPC helpers for panel visibility and work area hints.

use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use serde_json::Value;
use tracing::warn;
use unixnotis_core::Margins;

use crate::dbus::UiEvent;

/// Start a Hyprland active-window watcher for click-away panel closing.
pub fn start_active_window_watcher(
    event_tx: async_channel::Sender<UiEvent>,
    panel_visible: Arc<AtomicBool>,
) -> bool {
    let signature = match env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        Ok(value) => value,
        Err(_) => return false,
    };
    let runtime_dir = match env::var("XDG_RUNTIME_DIR") {
        Ok(value) => value,
        Err(_) => return false,
    };

    thread::spawn(move || {
        let socket_path = format!("{runtime_dir}/hypr/{signature}/.socket2.sock");
        loop {
            match UnixStream::connect(&socket_path) {
                Ok(stream) => {
                    let mut reader = BufReader::new(stream);
                    let mut buffer = Vec::with_capacity(256);
                    loop {
                        buffer.clear();
                        match reader.read_until(b'\n', &mut buffer) {
                            Ok(0) => break,
                            Ok(_) => {
                                if !buffer.starts_with(b"activewindow") {
                                    continue;
                                }
                                if !panel_visible.load(Ordering::SeqCst) {
                                    continue;
                                }
                                // The UI thread validates click state before closing to avoid hover-only focus changes.
                                let _ = event_tx.try_send(UiEvent::ClickOutside);
                            }
                            Err(err) => {
                                warn!(?err, "hyprland event stream read failed");
                                break;
                            }
                        }
                    }
                    warn!("hyprland event stream ended, reconnecting in 1s");
                }
                Err(err) => {
                    warn!(?err, "failed to connect to hyprland socket, retrying in 1s");
                }
            }
            thread::sleep(std::time::Duration::from_secs(1));
        }
    });

    true
}

/// Query Hyprland reserved work area for a specific output.
pub fn refresh_reserved_work_area(
    output: Option<String>,
    event_tx: async_channel::Sender<UiEvent>,
) {
    thread::spawn(move || {
        let reserved = reserved_work_area_sync(output.as_deref());
        let _ = event_tx.try_send(UiEvent::WorkAreaUpdated(reserved));
    });
}

fn reserved_work_area_sync(output: Option<&str>) -> Option<Margins> {
    let response = match send_command("j/monitors") {
        Ok(response) => response,
        Err(err) => {
            warn!(?err, "failed to query hyprland monitors");
            return None;
        }
    };
    let value: Value = serde_json::from_str(&response).ok()?;
    let monitors = value.as_array()?;
    for monitor in monitors {
        let Some(name) = monitor.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some(output_name) = output {
            if output_name != name {
                continue;
            }
        }
        let Some(reserved) = monitor.get("reserved") else {
            continue;
        };
        return parse_reserved(reserved);
    }
    None
}

fn parse_reserved(value: &Value) -> Option<Margins> {
    if let Some(array) = value.as_array() {
        if array.len() == 4 {
            let top = array[0].as_i64()? as i32;
            let bottom = array[1].as_i64()? as i32;
            let left = array[2].as_i64()? as i32;
            let right = array[3].as_i64()? as i32;
            return Some(Margins {
                top,
                right,
                bottom,
                left,
            });
        }
    }
    if let Some(object) = value.as_object() {
        let top = object.get("top").and_then(Value::as_i64)? as i32;
        let right = object.get("right").and_then(Value::as_i64)? as i32;
        let bottom = object.get("bottom").and_then(Value::as_i64)? as i32;
        let left = object.get("left").and_then(Value::as_i64)? as i32;
        return Some(Margins {
            top,
            right,
            bottom,
            left,
        });
    }
    None
}

fn send_command(command: &str) -> std::io::Result<String> {
    let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
    let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    if signature.is_empty() || runtime_dir.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Hyprland environment not available",
        ));
    }
    let socket_path = format!("{runtime_dir}/hypr/{signature}/.socket.sock");
    let mut stream = UnixStream::connect(&socket_path)?;
    let request = format!("{command}\n");
    stream.write_all(request.as_bytes())?;
    stream.flush()?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}
