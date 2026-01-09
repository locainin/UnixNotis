//! Notification daemon detection for install workflows.

use std::fs;
use std::path::Path;
use std::process::Command;

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

struct KnownDaemon {
    name: &'static str,
    unit: &'static str,
}

const KNOWN_DAEMONS: &[KnownDaemon] = &[
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
];

pub fn detect() -> Detection {
    let owner = detect_owner();
    let daemons = detect_known_daemons(&owner);
    Detection { owner, daemons }
}

fn detect_owner() -> Option<OwnerInfo> {
    let output = Command::new("busctl")
        .args(["--user", "status", "org.freedesktop.Notifications"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let status = String::from_utf8_lossy(&output.stdout);
    let mut comm = None;
    let mut pid = None;

    for line in status.lines() {
        if let Some(value) = line.strip_prefix("Comm=") {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                comm = Some(trimmed.to_string());
            }
        }
        if let Some(value) = line.strip_prefix("PID=") {
            let trimmed = value.trim();
            if let Ok(parsed) = trimmed.parse::<u32>() {
                pid = Some(parsed);
            }
        }
    }

    if comm.is_none() && pid.is_none() {
        return None;
    }

    let comm = pid
        .and_then(read_cmdline_program)
        .or_else(|| comm.or_else(|| pid.and_then(read_comm)));
    Some(OwnerInfo { pid, comm })
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
        Err(err) => (false, Some(err.to_string())),
    }
}

fn pgrep_exact(name: &str) -> Vec<u32> {
    let output = Command::new("pgrep").arg("-x").arg(name).output();
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

fn read_comm(pid: u32) -> Option<String> {
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
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn read_cmdline_program(pid: u32) -> Option<String> {
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
