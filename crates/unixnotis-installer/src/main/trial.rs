//! Trial run build, launch, PATH shim, and cleanup helpers

use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};

pub(crate) fn run_trial(repo_root: PathBuf) -> Result<()> {
    println!("Starting UnixNotis trial run.");
    println!("Press Ctrl+C to stop and restore the previous daemon.");

    // Build every runtime binary before launching the daemon
    // Trial auth depends on these paths existing in the same target tree
    let build_status = std::process::Command::new("cargo")
        .args([
            "build",
            "-p",
            "unixnotis-daemon",
            "-p",
            "unixnotis-popups",
            "-p",
            "unixnotis-center",
            "-p",
            "noticenterctl",
        ])
        .current_dir(&repo_root)
        .status()
        .map_err(|err| anyhow!("failed to build trial binaries: {}", err))?;
    if !build_status.success() {
        // Stop here so a stale daemon binary is never launched by accident
        return Err(anyhow!("trial build exited with failure"));
    }

    // Trial mode runs debug binaries because this path supports local edit-test loops
    let daemon_bin = repo_root
        .join("target")
        .join("debug")
        .join("unixnotis-daemon");
    let ctl_bin = repo_root.join("target").join("debug").join("noticenterctl");
    if !daemon_bin.is_file() {
        // Missing build output means cargo did not produce what the trial launcher needs
        return Err(anyhow!(
            "trial daemon binary not found at {}",
            daemon_bin.display()
        ));
    }
    if !ctl_bin.is_file() {
        // The control binary must exist so keybinds and manual calls can reach the trial daemon
        return Err(anyhow!(
            "trial control binary not found at {}",
            ctl_bin.display()
        ));
    }

    println!("Trial control binary: {}", ctl_bin.display());
    // A temporary shim is optional; direct binary usage remains the fallback
    let trial_ctl_shim = ensure_trial_control_access(&ctl_bin)?;

    let status = if let Some(shim) = trial_ctl_shim.as_ref() {
        // Use a shell wrapper only when there is a file that needs signal-time cleanup
        run_trial_with_shim_cleanup(&daemon_bin, &shim.path, &shim.target)?
    } else {
        // No shim means no extra filesystem cleanup is needed
        std::process::Command::new(&daemon_bin)
            .args(["--trial", "--restore", "auto", "--yes"])
            .status()
            .map_err(|err| anyhow!("failed to run trial: {}", err))?
    };

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("trial run exited with failure"))
    }
}

struct TrialControlShim {
    // Path is kept so Drop can remove exactly the trial-owned file
    path: PathBuf,
    // Target proves the shim still points at the debug control binary created by this run
    target: PathBuf,
}

impl Drop for TrialControlShim {
    fn drop(&mut self) {
        // Best-effort cleanup keeps trial-only shim files from lingering after exit
        let _ = remove_trial_control_shim(&self.path, &self.target);
    }
}

fn ensure_trial_control_access(ctl_bin: &Path) -> Result<Option<TrialControlShim>> {
    // PATH order decides which noticenterctl a shell command will actually run
    let path_entries = path_entries();
    let existing = find_command_on_path_with_index("noticenterctl", &path_entries);
    if existing
        .as_ref()
        .is_some_and(|(_, path)| trial_control_command_is_compatible(path, ctl_bin))
    {
        // Existing command already maps to a daemon-trusted trial control path
        return Ok(None);
    }

    // Relaxed daemon auth only trusts ~/.local/bin outside the target tree
    let preferred_dir = env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".local").join("bin"));
    let shim_dir = preferred_dir
        .as_deref()
        .and_then(|dir| select_trial_shim_dir(dir, &path_entries, existing.as_ref()));

    let Some(shim_dir) = shim_dir else {
        if let Some((_, path)) = existing.as_ref() {
            // Do not create a shim that cannot win PATH lookup or daemon auth
            println!("Found non-trial control command at {}", path.display());
            println!("Trial mode will not add a shim that stays shadowed or untrusted");
        } else {
            println!("No trusted PATH location was found for a temporary trial noticenterctl");
        }
        println!("Use {} directly during trial", ctl_bin.display());
        return Ok(None);
    };

    let shim_path = shim_dir.join("noticenterctl");
    if path_exists_no_follow(&shim_path) {
        // Never overwrite normal installed usage with a temporary trial link
        println!(
            "Trial control command is not visible on PATH, and {} already exists",
            shim_path.display()
        );
        println!("Use {} directly during trial", ctl_bin.display());
        return Ok(None);
    }

    let target = canonicalize_best_effort(ctl_bin);

    #[cfg(unix)]
    {
        // Symlink keeps the shim small and follows rebuilds of the debug control binary
        unix_fs::symlink(&target, &shim_path).map_err(|err| {
            anyhow!(
                "failed to create trial noticenterctl shim at {}: {}",
                shim_path.display(),
                err
            )
        })?;
    }
    #[cfg(not(unix))]
    {
        // Non-Unix targets do not have the same symlink assumptions
        fs::copy(&target, &shim_path).map_err(|err| {
            anyhow!(
                "failed to copy trial noticenterctl shim to {}: {}",
                shim_path.display(),
                err
            )
        })?;
    }

    println!(
        "Temporarily linked trial noticenterctl at {}",
        shim_path.display()
    );

    Ok(Some(TrialControlShim {
        path: shim_path,
        target,
    }))
}

pub(super) fn select_trial_shim_dir(
    preferred_dir: &Path,
    path_entries: &[PathBuf],
    existing: Option<&(usize, PathBuf)>,
) -> Option<PathBuf> {
    // The preferred dir must be on PATH or shell commands cannot see the shim
    let preferred_index = path_entries
        .iter()
        .position(|entry| path_entries_match(entry, preferred_dir))?;

    // Trial auth only trusts ~/.local/bin outside the build tree, so skip every
    // other writable PATH directory even if it would be earlier
    if let Some((existing_index, _)) = existing {
        // If an older command wins PATH resolution before ~/.local/bin, a shim
        // here would never be observed by the shell
        if *existing_index < preferred_index {
            return None;
        }
    }

    if !preferred_dir.exists() {
        // Creating ~/.local/bin is safe only after confirming the path can matter
        fs::create_dir_all(preferred_dir)
            .map_err(|err| anyhow!("failed to create {}: {}", preferred_dir.display(), err))
            .ok()?;
    }
    if !preferred_dir.is_dir() || !path_dir_is_writable(preferred_dir) {
        // A non-directory or read-only location cannot host a temporary shim
        return None;
    }

    Some(preferred_dir.to_path_buf())
}

fn find_command_on_path_with_index(command: &str, entries: &[PathBuf]) -> Option<(usize, PathBuf)> {
    // Return the first command because that is what shell lookup will execute
    entries.iter().enumerate().find_map(|(index, entry)| {
        let candidate = entry.join(command);
        if candidate.is_file() {
            Some((index, candidate))
        } else {
            None
        }
    })
}

fn trial_control_command_is_compatible(path: &Path, ctl_bin: &Path) -> bool {
    // Canonical comparison handles symlinks without trusting a raw path string
    let canonical = canonicalize_best_effort(path);
    if canonical == canonicalize_best_effort(ctl_bin) {
        return true;
    }

    // Trial auth also trusts ~/.local/bin/noticenterctl
    let local_bin = env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".local").join("bin").join("noticenterctl"));
    if local_bin
        .as_deref()
        .is_some_and(|candidate| canonicalize_best_effort(candidate) == canonical)
    {
        return true;
    }

    // Trial auth trusts target/debug and target/release siblings under the same target root
    let Some(profile_dir) = ctl_bin.parent() else {
        // A control binary without a profile dir cannot prove target-tree ancestry
        return false;
    };
    let Some(target_root) = profile_dir.parent() else {
        // The expected layout is target/<profile>/noticenterctl
        return false;
    };
    ["debug", "release"]
        .iter()
        .map(|profile| target_root.join(profile).join("noticenterctl"))
        .any(|candidate| canonicalize_best_effort(&candidate) == canonical)
}

fn path_entries() -> Vec<PathBuf> {
    // Empty PATH is treated as no available shell command locations
    let Ok(path_var) = env::var("PATH") else {
        return Vec::new();
    };
    env::split_paths(&path_var).collect()
}

pub(super) fn path_entries_match(left: &Path, right: &Path) -> bool {
    // Fast path avoids filesystem work for normal exact entries
    if left == right {
        return true;
    }
    // Canonical comparison lets symlinked PATH entries match the real directory
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(lhs), Ok(rhs)) => lhs == rhs,
        _ => false,
    }
}

fn path_dir_is_writable(dir: &Path) -> bool {
    // create_new avoids touching any existing file in the target directory
    let probe = dir.join(format!(
        ".unixnotis-trial-write-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            // Probe file is trial-only and should not outlive the writability check
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

pub(super) fn path_exists_no_follow(path: &Path) -> bool {
    // symlink_metadata catches dangling symlinks that exists() would miss
    fs::symlink_metadata(path).is_ok()
}

pub(super) fn remove_trial_control_shim(path: &Path, expected_target: &Path) -> Result<bool> {
    #[cfg(unix)]
    {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(err) => {
                return Err(anyhow!(
                    "failed to inspect trial noticenterctl shim at {}: {}",
                    path.display(),
                    err
                ));
            }
        };
        if !metadata.file_type().is_symlink() {
            // A replaced regular file is user state, not trial-owned cleanup state
            return Ok(false);
        }
        let target = fs::read_link(path).map_err(|err| {
            anyhow!(
                "failed to inspect trial noticenterctl shim target at {}: {}",
                path.display(),
                err
            )
        })?;
        if !trial_shim_target_matches(path, &target, expected_target) {
            return Ok(false);
        }
        fs::remove_file(path).map_err(|err| {
            anyhow!(
                "failed to remove trial noticenterctl shim at {}: {}",
                path.display(),
                err
            )
        })?;
        Ok(true)
    }

    #[cfg(not(unix))]
    {
        let _ = expected_target;
        // Non-Unix fallback creates a copied file, so normal Drop cleanup remains best-effort
        match fs::remove_file(path) {
            Ok(()) => Ok(true),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(anyhow!(
                "failed to remove trial noticenterctl shim at {}: {}",
                path.display(),
                err
            )),
        }
    }
}

#[cfg(unix)]
fn trial_shim_target_matches(path: &Path, raw_target: &Path, expected_target: &Path) -> bool {
    let resolved = if raw_target.is_absolute() {
        raw_target.to_path_buf()
    } else {
        path.parent()
            .map(|parent| parent.join(raw_target))
            .unwrap_or_else(|| raw_target.to_path_buf())
    };
    canonicalize_best_effort(&resolved) == canonicalize_best_effort(expected_target)
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    // Missing paths still need stable comparison behavior
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

fn run_trial_with_shim_cleanup(
    daemon_bin: &Path,
    shim_path: &Path,
    expected_target: &Path,
) -> Result<std::process::ExitStatus> {
    // Shell trap ensures shim cleanup still happens when Ctrl+C kills the installer process
    let daemon = shell_quote(daemon_bin.display().to_string().as_str());
    let shim = shell_quote(shim_path.display().to_string().as_str());
    let target = shell_quote(expected_target.display().to_string().as_str());
    // Trap cleanup covers the common signal path where Rust Drop may not run
    let script = trial_launch_script(&daemon, &shim, &target);
    std::process::Command::new("sh")
        .arg("-c")
        .arg(script)
        .status()
        .map_err(|err| anyhow!("failed to run trial: {}", err))
}

pub(super) fn trial_launch_script(daemon: &str, shim: &str, target: &str) -> String {
    format!(
        "cleanup() {{ if [ -L {shim} ] && [ \"$(readlink -- {shim})\" = {target} ]; then rm -f -- {shim}; fi; }}; trap cleanup EXIT INT TERM; {daemon} --trial --restore auto --yes"
    )
}

fn shell_quote(value: &str) -> String {
    // Single-quote shell escaping keeps paths with spaces or quotes intact
    let mut quoted = String::from("'");
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\"'\"'");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}
