//! Session environment evidence used to explain D-Bus connection failures

use std::env;
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;

use super::super::report::{DoctorCheck, DoctorSeverity};

pub(in crate::doctor) fn inspect_session_environment(bus_connected: bool) -> DoctorCheck {
    // Runtime path shape is useful context without exposing the literal user path
    let runtime = env::var_os("XDG_RUNTIME_DIR").filter(|value| !value.is_empty());
    let runtime_path = runtime.as_ref().map(PathBuf::from);
    let runtime_present = runtime_path.is_some();
    let runtime_absolute = runtime_path.as_ref().is_some_and(|path| path.is_absolute());
    let runtime_directory = runtime_path.as_ref().is_some_and(|path| path.is_dir());
    let runtime_bus_socket = runtime_path
        .as_ref()
        .map(|path| path.join("bus"))
        .and_then(|path| std::fs::symlink_metadata(path).ok())
        .is_some_and(|metadata| metadata.file_type().is_socket());

    // Only the transport prefix is retained because the address can carry private paths
    let bus_address = env::var("DBUS_SESSION_BUS_ADDRESS")
        .ok()
        .filter(|value| !value.is_empty());
    let bus_transport = bus_address
        .as_deref()
        .and_then(|value| value.split_once(':').map(|(transport, _)| transport))
        .unwrap_or("unset");
    let wayland = env::var_os("WAYLAND_DISPLAY").is_some_and(|value| !value.is_empty())
        || env::var("XDG_SESSION_TYPE").is_ok_and(|value| value.eq_ignore_ascii_case("wayland"));

    // Independent notes preserve every likely session setup problem in one check
    let anomalies = [
        (!runtime_present).then_some("XDG_RUNTIME_DIR is not set"),
        (runtime_present && !runtime_absolute).then_some("XDG_RUNTIME_DIR is not absolute"),
        (runtime_absolute && !runtime_directory).then_some("XDG_RUNTIME_DIR is not a directory"),
        bus_address
            .is_none()
            .then_some("DBUS_SESSION_BUS_ADDRESS is not set"),
        (!runtime_bus_socket).then_some("the runtime bus socket is not present"),
        (!wayland).then_some("a Wayland session was not detected"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    // A working bus downgrades environment anomalies because transport already succeeded
    let severity = if anomalies.is_empty() {
        DoctorSeverity::Pass
    } else if bus_connected {
        DoctorSeverity::Note
    } else {
        DoctorSeverity::Warning
    };
    let summary = if anomalies.is_empty() {
        "Session environment contains the expected D-Bus inputs"
    } else if bus_connected {
        "Session bus connected despite environment anomalies"
    } else {
        "Session environment may explain the D-Bus connection failure"
    };
    // Report booleans and classifications instead of raw environment values
    let details = format!(
        "XDG_RUNTIME_DIR: {}\nDBUS_SESSION_BUS_ADDRESS: {}, {} transport\nRuntime bus socket: {}\nWayland session: {}",
        runtime_description(runtime_present, runtime_absolute, runtime_directory),
        if bus_address.is_some() { "present" } else { "missing" },
        bus_transport,
        if runtime_bus_socket { "present" } else { "missing" },
        if wayland { "detected" } else { "not detected" },
    );
    let mut check = DoctorCheck::new(
        "environment.session",
        "Session environment",
        severity,
        summary,
    )
    .details(details)
    .data("xdg_runtime_dir_present", runtime_present)
    .data("xdg_runtime_dir_absolute", runtime_absolute)
    .data("xdg_runtime_dir_directory", runtime_directory)
    .data("dbus_address_present", bus_address.is_some())
    .data("dbus_transport", bus_transport)
    .data("runtime_bus_socket_present", runtime_bus_socket)
    .data("wayland_detected", wayland);
    // Hints become actionable only when the environment may explain a real failure
    if !bus_connected && !anomalies.is_empty() {
        check = check.hint(anomalies.join("; "));
    }
    check
}

const fn runtime_description(present: bool, absolute: bool, directory: bool) -> &'static str {
    match (present, absolute, directory) {
        (false, _, _) => "missing",
        (true, false, _) => "present, relative",
        (true, true, false) => "present, absolute, not a directory",
        (true, true, true) => "present, absolute, directory",
    }
}
