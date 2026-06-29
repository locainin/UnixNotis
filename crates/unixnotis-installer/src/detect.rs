//! Notification daemon detection for install workflows.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

use rustix::process::geteuid;
use serde_json::Value;

#[derive(Clone)]
pub struct OwnerInfo {
    pub pid: Option<u32>,
    pub comm: Option<String>,
}

#[derive(Clone)]
pub struct DetectedDaemon {
    pub name: String,
    pub unit: String,
    pub systemd_active: bool,
    pub systemd_error: Option<String>,
    pub running_pids: Vec<u32>,
    pub is_owner: bool,
}

#[derive(Clone)]
pub struct Detection {
    pub owner: Option<OwnerInfo>,
    pub daemons: Vec<DetectedDaemon>,
}

pub(crate) struct KnownDaemon {
    pub(crate) name: &'static str,
    pub(crate) unit: &'static str,
}

pub(crate) const KNOWN_DAEMONS: &[KnownDaemon] = &[
    KnownDaemon {
        name: "unixnotis-daemon",
        unit: "unixnotis-daemon.service",
    },
    KnownDaemon {
        name: "mako",
        unit: "mako.service",
    },
    KnownDaemon {
        name: "dunst",
        unit: "dunst.service",
    },
    KnownDaemon {
        name: "swaync",
        unit: "swaync.service",
    },
    KnownDaemon {
        name: "notify-osd",
        unit: "notify-osd.service",
    },
    KnownDaemon {
        name: "quickshell",
        unit: "quickshell.service",
    },
    KnownDaemon {
        name: "hyprnotify",
        unit: "hyprnotify.service",
    },
    KnownDaemon {
        name: "fnott",
        unit: "fnott.service",
    },
];

pub fn detect() -> Detection {
    let owner = detect_owner();
    let daemons = detect_known_daemons(&owner);
    Detection { owner, daemons }
}

pub(crate) fn parse_busctl_status(status: &str) -> Option<OwnerInfo> {
    // Parses `busctl --user status` output and tolerates the indented key/value format.
    let mut comm = None;
    let mut pid = None;

    for line in status.lines() {
        let trimmed = line.trim_start();
        let Some((raw_key, raw_value)) = trimmed.split_once('=') else {
            continue;
        };
        // Normalize key/value parsing to accept both `Key=Value` and `Key = Value` variants.
        let key = raw_key.trim();
        let value = raw_value.trim();
        if value.is_empty() {
            // Empty values are ignored to avoid masking earlier valid data.
            continue;
        }
        match key {
            "Comm" => {
                // Preserve the reported command name for fallback owner matching.
                comm = Some(value.to_string());
            }
            "PID" => {
                if let Ok(parsed) = value.parse::<u32>() {
                    // PID 0 is not a valid user process; ignore it to avoid false positives.
                    if parsed != 0 {
                        pid = Some(parsed);
                    }
                }
            }
            _ => {}
        }
    }

    if comm.is_none() && pid.is_none() {
        return None;
    }

    Some(OwnerInfo { pid, comm })
}

pub(crate) fn parse_busctl_json(status: &str) -> Option<OwnerInfo> {
    // Accept loosely structured JSON and search for PID/Comm fields anywhere in the tree.
    let value: Value = serde_json::from_str(status).ok()?;
    let mut comm = None;
    let mut pid = None;
    walk_busctl_json(&value, &mut comm, &mut pid);

    if comm.is_none() && pid.is_none() {
        return None;
    }

    Some(OwnerInfo { pid, comm })
}

fn walk_busctl_json(value: &Value, comm: &mut Option<String>, pid: &mut Option<u32>) {
    match value {
        Value::Object(map) => {
            for (key, value) in map {
                if key == "Comm" && comm.is_none() {
                    if let Value::String(text) = value {
                        if !text.trim().is_empty() {
                            *comm = Some(text.to_string());
                        }
                    }
                }
                if key == "PID" && pid.is_none() {
                    if let Some(parsed) = parse_pid_value(value) {
                        *pid = Some(parsed);
                    }
                }
                walk_busctl_json(value, comm, pid);
            }
        }
        Value::Array(items) => {
            for item in items {
                walk_busctl_json(item, comm, pid);
            }
        }
        _ => {}
    }
}

fn parse_pid_value(value: &Value) -> Option<u32> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|val| {
            if val == 0 {
                None
            } else {
                u32::try_from(val).ok()
            }
        }),
        Value::String(text) => {
            text.parse::<u32>()
                .ok()
                .and_then(|val| if val == 0 { None } else { Some(val) })
        }
        _ => None,
    }
}

fn detect_owner() -> Option<OwnerInfo> {
    let OwnerInfo { pid, comm } = read_busctl_owner()?;
    // Prefer the executable name derived from argv0; fall back to busctl and /proc data.
    let comm = pid
        .and_then(read_cmdline_program)
        .or_else(|| comm.or_else(|| pid.and_then(read_comm)));
    Some(OwnerInfo { pid, comm })
}

fn read_busctl_owner() -> Option<OwnerInfo> {
    // Prefer JSON output when supported; fall back to the textual format otherwise.
    if let Some(status) = run_busctl(&[
        "--user",
        "--json=short",
        "status",
        "org.freedesktop.Notifications",
    ]) {
        if let Some(owner) = parse_busctl_json(&status) {
            return Some(owner);
        }
    }

    let status = run_busctl(&["--user", "status", "org.freedesktop.Notifications"])?;
    parse_busctl_status(&status)
}

fn run_busctl(args: &[&str]) -> Option<String> {
    let output = Command::new("busctl").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn detect_known_daemons(owner: &Option<OwnerInfo>) -> Vec<DetectedDaemon> {
    let owner_name = owner.as_ref().and_then(|info| info.comm.as_deref());
    KNOWN_DAEMONS
        .iter()
        .map(|daemon| {
            let (systemd_active, systemd_error) = is_unit_active(daemon.unit);
            DetectedDaemon {
                name: daemon.name.to_string(),
                unit: daemon.unit.to_string(),
                systemd_active,
                systemd_error,
                running_pids: pgrep_exact(daemon.name),
                is_owner: owner_name == Some(daemon.name),
            }
        })
        .collect()
}

fn is_unit_active(unit: &str) -> (bool, Option<String>) {
    match Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", unit])
        .status()
    {
        Ok(status) => (status.success(), None),
        Err(err) => (false, systemctl_spawn_error(&err)),
    }
}

pub(crate) fn systemctl_spawn_error(err: &std::io::Error) -> Option<String> {
    if err.kind() == ErrorKind::NotFound {
        // Non-systemd systems often do not install systemctl; pgrep/D-Bus still give signal
        None
    } else {
        Some(err.to_string())
    }
}

fn pgrep_exact(name: &str) -> Vec<u32> {
    // Limit process discovery to the current user to avoid cross-user noise.
    let uid = geteuid().as_raw();
    let output = Command::new("pgrep")
        .args(["-x", "-u", &uid.to_string(), name])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect()
}

pub(crate) fn read_comm(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/comm", pid);
    if let Ok(contents) = fs::read_to_string(path) {
        let comm = contents.trim().to_string();
        if !comm.is_empty() {
            return Some(comm);
        }
    }
    let output = Command::new("ps")
        .arg("-p")
        .arg(pid.to_string())
        .arg("-o")
        .arg("comm=")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let comm = String::from_utf8_lossy(&output.stdout);
    // Avoid returning empty command names from ps output.
    let comm = comm.trim();
    if comm.is_empty() {
        None
    } else {
        Some(comm.to_string())
    }
}

pub(crate) fn read_cmdline_program(pid: u32) -> Option<String> {
    let path = format!("/proc/{}/cmdline", pid);
    let contents = fs::read(path).ok()?;
    let mut parts = contents
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty());
    let program = parts.next()?;
    let program = String::from_utf8_lossy(program);
    let name = Path::new(program.as_ref())
        .file_name()
        .and_then(|name| name.to_str())?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
