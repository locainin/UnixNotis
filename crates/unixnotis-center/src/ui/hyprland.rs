//! Hyprland IPC helpers for panel visibility and work area hints.

use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use serde_json::Value;
use tracing::{debug, warn};
use unixnotis_core::{util, Margins};

use crate::dbus::UiEvent;

/// Start a Hyprland active-window watcher for click-away panel closing.
pub fn start_active_window_watcher(
    event_tx: async_channel::Sender<UiEvent>, // Channel used to notify the GTK/UI thread about events detected here.
    panel_visible: Arc<AtomicBool>, // Shared flag: true when the panel is currently visible (open), false when hidden.
) -> bool {
    // Hyprland sets HYPRLAND_INSTANCE_SIGNATURE for each compositor instance.
    // Without it, we can't derive the correct socket path, so we fail fast and return false.
    let signature = match env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        Ok(value) => value,
        Err(_) => return false, // Not running under Hyprland (or env not exported); caller can fall back to other behavior.
    };

    // Hyprland sockets live under the per-user runtime directory ($XDG_RUNTIME_DIR).
    // Without XDG_RUNTIME_DIR the socket path cannot be constructed, so return false.
    let runtime_dir = match env::var("XDG_RUNTIME_DIR") {
        Ok(value) => value,
        Err(_) => return false, // Environment is missing required runtime dir; treat watcher as unavailable.
    };

    // Run the Hyprland event stream reader on a dedicated OS thread:
    // - avoids blocking the GTK main loop
    // - keeps the logic simple (blocking I/O is fine here)
    // - isolates reconnect logic away from UI code
    thread::spawn(move || {
        // Hyprland's event socket (socket2) is a newline-delimited text stream of compositor events.
        // This path format is Hyprland-specific and derived from runtime_dir + instance signature.
        let socket_path = format!("{runtime_dir}/hypr/{signature}/.socket2.sock");

        // Outer loop: connect -> read until failure -> sleep -> reconnect.
        // This makes the watcher resilient to Hyprland restarts, socket restarts, or transient errors.
        loop {
            match UnixStream::connect(&socket_path) {
                Ok(stream) => {
                    // Wrap the UnixStream in a buffered reader to efficiently read line-delimited events.
                    // Hyprland emits events as ASCII-ish lines ending in '\n'.
                    let mut reader = BufReader::new(stream);

                    // Reusable buffer for each line read:
                    // - avoids reallocations per event
                    // - small default capacity is enough for typical event lines
                    let mut buffer = Vec::with_capacity(256);

                    // Inner loop: read events from the connected stream until EOF/error.
                    loop {
                        // Clear but keep capacity so the Vec backing allocation is reused.
                        buffer.clear();

                        // Read a single event line (up to and including '\n') into buffer.
                        // read_until appends into the Vec, so we must clear it first.
                        match reader.read_until(b'\n', &mut buffer) {
                            Ok(0) => break, // EOF: Hyprland closed the stream; exit inner loop and reconnect.
                            Ok(_) => {
                                // Filter: we only care about active-window changes.
                                // Hyprland emits many event types; ignoring others reduces work.
                                if !buffer.starts_with(b"activewindow") {
                                    continue;
                                }

                                // If the panel isn't visible, we don't want to close it (it's already closed).
                                // This also prevents needless event traffic into the UI thread when hidden.
                                // SeqCst is conservative; correctness is more important than micro-optimizing this.
                                if !panel_visible.load(Ordering::SeqCst) {
                                    continue;
                                }

                                // The UI thread validates click state before closing to avoid hover-only focus changes.
                                // This thread only signals that "activewindow changed while panel visible";
                                // the UI will decide whether that implies a click-away close.
                                let _ = event_tx.try_send(UiEvent::ClickOutside);
                                // try_send is used deliberately:
                                // - we never want to block this thread on UI backpressure
                                // - if the channel is full (unlikely), dropping this event is acceptable because
                                //   subsequent activewindow events will arrive and the UI still validates state.
                            }
                            Err(err) => {
                                // Any read error means the stream is unhealthy; log and reconnect.
                                warn!(?err, "hyprland event stream read failed");
                                break;
                            }
                        }
                    }

                    // If we exit the inner loop, the stream ended or errored out.
                    // Sleep a bit before reconnect to avoid busy reconnection loops during compositor restart.
                    warn!("hyprland event stream ended, reconnecting in 1s");
                }
                Err(err) => {
                    // Initial connect failed (Hyprland not ready yet, socket missing, permission issue, etc.).
                    // Sleep before retrying to avoid burning CPU.
                    warn!(?err, "failed to connect to hyprland socket, retrying in 1s");
                }
            }

            // Backoff between reconnect attempts.
            // Keeps CPU usage near zero when Hyprland isn't available or is restarting.
            thread::sleep(std::time::Duration::from_secs(1));
        }
    });

    // Returning true indicates the watcher was successfully started (at least to the point of spawning the thread).
    // The thread itself will handle reconnect attempts and may still fail to connect if Hyprland isn't running.
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
    let value: Value = match serde_json::from_str(&response) {
        Ok(value) => value,
        Err(err) => {
            let snippet = util::log_snippet(&response);
            warn!(
                ?err,
                response = %snippet,
                response_len = response.len(),
                "failed to parse hyprland monitors JSON"
            );
            return None;
        }
    };
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
    // Hyprland "reserved" can show up either as a 4-element array or (in some contexts/tools)
    // as an object with explicit keys. Support both to be robust across versions/tools.
    if let Some(array) = value.as_array() {
        if array.len() == 4 {
            // Hyprland JSON monitor output emits reserved as [top, right, bottom, left].
            // Normalize into our internal Margins { top, right, bottom, left } ordering.
            let top = array[0].as_i64()?.max(0) as i32;
            let right = array[1].as_i64()?.max(0) as i32;
            let bottom = array[2].as_i64()?.max(0) as i32;
            let left = array[3].as_i64()?.max(0) as i32;

            // Debug log helps validate order on real systems (especially multi-monitor).
            debug!(left, top, right, bottom, "hyprland reserved margins parsed");

            return Some(Margins {
                top,
                right,
                bottom,
                left,
            });
        }
    }

    if let Some(object) = value.as_object() {
        // Object form is unambiguous; just read the named edges.
        // Using and_then(Value::as_i64) ensures type correctness; any mismatch returns None.
        let top = object.get("top").and_then(Value::as_i64)?.max(0) as i32;
        let right = object.get("right").and_then(Value::as_i64)?.max(0) as i32;
        let bottom = object.get("bottom").and_then(Value::as_i64)?.max(0) as i32;
        let left = object.get("left").and_then(Value::as_i64)?.max(0) as i32;

        debug!(
            left,
            top, right, bottom, "hyprland reserved margins parsed from object"
        );

        return Some(Margins {
            top,
            right,
            bottom,
            left,
        });
    }

    // Unknown shape (not array/object) or missing/invalid fields.
    None
}

fn send_command(command: &str) -> std::io::Result<String> {
    // Hyprland exposes its IPC socket via XDG_RUNTIME_DIR + HYPRLAND_INSTANCE_SIGNATURE.
    // If these env vars aren't present, we're not in a Hyprland session (or IPC isn't available).
    let signature = env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
    let runtime_dir = env::var("XDG_RUNTIME_DIR").unwrap_or_default();
    if signature.is_empty() || runtime_dir.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Hyprland environment not available",
        ));
    }

    // ".socket.sock" is Hyprland's request/response command socket (not the event stream).
    let socket_path = format!("{runtime_dir}/hypr/{signature}/.socket.sock");
    let mut stream = UnixStream::connect(&socket_path)?;

    // Hyprland expects newline-terminated commands on this socket.
    let request = format!("{command}\n");
    stream.write_all(request.as_bytes())?;
    stream.flush()?; // Make sure the command is sent immediately.

    // Hyprland replies with a plain-text response; read it fully until EOF.
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::parse_reserved;

    #[test]
    fn parse_reserved_array_order() {
        // [top, right, bottom, left] -> Margins { top, right, bottom, left }
        let value = serde_json::json!([10, 20, 30, 40]);
        let margins = parse_reserved(&value).expect("reserved margins");
        assert_eq!(margins.top, 10);
        assert_eq!(margins.right, 20);
        assert_eq!(margins.bottom, 30);
        assert_eq!(margins.left, 40);
    }
}
