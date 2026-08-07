//! Trusted Rust toolchain executable discovery

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
use std::ffi::{CStr, OsStr};
#[cfg(unix)]
use std::io;
#[cfg(unix)]
use std::mem::MaybeUninit;
#[cfg(unix)]
use std::os::unix::ffi::{OsStrExt, OsStringExt};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use std::ptr;

use anyhow::{anyhow, Result};

use unixnotis_core::util::TRUSTED_SYSTEM_TOOL_DIRS;

#[cfg(unix)]
const PASSWD_BUFFER_START: usize = 1024;
#[cfg(unix)]
const PASSWD_BUFFER_LIMIT: usize = 1024 * 1024;
const CARGO_EXECUTION_ENV_VARS: [&str; 13] = [
    "RUSTC",
    "RUSTDOC",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTDOC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTFLAGS",
    "CARGO_BUILD_ENCODED_RUSTFLAGS",
    "RUSTUP_TOOLCHAIN",
    "RUSTFLAGS",
    "CARGO_ENCODED_RUSTFLAGS",
];

#[derive(Clone, Debug)]
struct ResolvedToolchain {
    cargo: PathBuf,
    rustc: PathBuf,
    rustdoc: PathBuf,
    path: OsString,
}

struct ValidatedExecutable {
    launch_path: PathBuf,
    canonical_path: PathBuf,
}

/// Resolve Cargo without consulting the inherited PATH
pub fn resolve_cargo() -> Result<PathBuf> {
    Ok(resolve_toolchain()?.cargo)
}

fn resolve_toolchain() -> Result<ResolvedToolchain> {
    let home = account_home_dir()?;
    let cargo = resolve_cargo_path(&home)?;
    resolve_toolchain_from_cargo(&home, &cargo)
}

fn resolve_cargo_path(home: &Path) -> Result<PathBuf> {
    let mut candidates = vec![home.join(".cargo").join("bin").join("cargo")];
    candidates.extend(
        TRUSTED_SYSTEM_TOOL_DIRS
            .iter()
            .map(|directory| Path::new(directory).join("cargo")),
    );

    for candidate in candidates {
        let Ok(validated) = validate_executable(&candidate) else {
            continue;
        };

        // Rustup proxy paths need one trusted lookup to bind Cargo to the selected toolchain
        if validated
            .canonical_path
            .file_name()
            .is_some_and(|name| name == "rustup")
        {
            if let Ok(cargo) = rustup_which(&validated.canonical_path, home, "cargo") {
                return Ok(cargo);
            }
            continue;
        }

        // Return the canonical executable rather than reopening a candidate symlink later
        return Ok(validated.canonical_path);
    }

    Err(anyhow!(
        "cargo was not found in the approved Rust toolchain locations"
    ))
}

/// Build a Cargo command with a stable argv[0] and a sanitized build environment
pub fn cargo_command(path: &Path) -> Result<Command> {
    let home = account_home_dir()?;
    let tools = resolve_toolchain_from_cargo(&home, path)?;
    let mut command = Command::new(&tools.cargo);

    #[cfg(unix)]
    {
        // Rustup uses argv[0] to distinguish Cargo from the rustup frontend
        command.arg0("cargo");
    }

    sanitize_command_environment(&mut command, &home, &tools.path);
    // Absolute compiler paths prevent Cargo from resolving rustc or rustdoc through PATH
    command.env("RUSTC", &tools.rustc);
    command.env("RUSTDOC", &tools.rustdoc);

    Ok(command)
}

fn resolve_toolchain_from_cargo(home: &Path, cargo: &Path) -> Result<ResolvedToolchain> {
    let cargo = validate_executable(cargo)?.canonical_path;
    let rustc = resolve_compiler_tool(home, &cargo, "rustc")?;
    let rustdoc = resolve_compiler_tool(home, &cargo, "rustdoc")?;
    let path = trusted_tool_path(&cargo, &rustc, &rustdoc)?;

    Ok(ResolvedToolchain {
        cargo,
        rustc,
        rustdoc,
        path,
    })
}

fn resolve_compiler_tool(home: &Path, cargo: &Path, tool: &str) -> Result<PathBuf> {
    if let Some(parent) = cargo.parent() {
        let sibling = parent.join(tool);
        if let Ok(validated) = validate_executable(&sibling) {
            // Keep proxy basenames such as rustc and rustdoc so rustup dispatches correctly
            if validated
                .canonical_path
                .file_name()
                .is_some_and(|name| name == "rustup")
            {
                return Ok(validated.launch_path);
            }
            return Ok(validated.canonical_path);
        }
    }

    let rustup = resolve_rustup(home, cargo)?;
    rustup_which(&rustup, home, tool)
}

fn resolve_rustup(home: &Path, cargo: &Path) -> Result<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(parent) = cargo.parent() {
        candidates.push(parent.join("rustup"));
    }
    candidates.push(home.join(".cargo").join("bin").join("rustup"));
    candidates.extend(
        TRUSTED_SYSTEM_TOOL_DIRS
            .iter()
            .map(|directory| Path::new(directory).join("rustup")),
    );

    candidates
        .into_iter()
        .find_map(|candidate| validate_executable(&candidate).ok())
        .map(|validated| validated.canonical_path)
        .ok_or_else(|| anyhow!("rustup was not found in the approved toolchain locations"))
}

fn rustup_which(rustup: &Path, home: &Path, tool: &str) -> Result<PathBuf> {
    let mut command = Command::new(rustup);
    #[cfg(unix)]
    command.arg0("rustup");
    command.args(["which", tool]);
    sanitize_command_environment(
        &mut command,
        home,
        &trusted_tool_path_from_directories(std::iter::empty())?,
    );

    let output = command
        .output()
        .map_err(|error| anyhow!("failed to resolve rustup {tool}: {error}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "rustup could not resolve {tool}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let path = PathBuf::from(
        String::from_utf8(output.stdout)
            .map_err(|error| anyhow!("rustup returned a non-UTF-8 {tool} path: {error}"))?
            .trim(),
    );
    Ok(validate_executable(&path)?.canonical_path)
}

fn sanitize_command_environment(command: &mut Command, home: &Path, path: &OsString) {
    // Cargo and rustup must not read configuration from environment-selected homes
    command.env("HOME", home);
    command.env("CARGO_HOME", home.join(".cargo"));
    // Rustup must use the account-owned toolchain registry selected by this resolver
    command.env("RUSTUP_HOME", home.join(".rustup"));
    // PATH is rebuilt from only validated toolchain and fixed system directories
    command.env("PATH", path);
    for variable in CARGO_EXECUTION_ENV_VARS {
        // These variables can replace rustc or wrap every compiler invocation
        command.env_remove(variable);
    }

    #[cfg(unix)]
    for (name, _value) in std::env::vars_os() {
        // Target-specific linker, runner, and flags variables are dynamically named
        if is_target_execution_variable(&name) {
            command.env_remove(name);
        }
    }
}

fn trusted_tool_path(cargo: &Path, rustc: &Path, rustdoc: &Path) -> Result<OsString> {
    let directories = [cargo, rustc, rustdoc]
        .into_iter()
        .filter_map(|path| path.parent())
        .map(validate_tool_directory)
        .collect::<Result<Vec<_>>>()?;
    trusted_tool_path_from_directories(directories)
}

fn trusted_tool_path_from_directories(
    directories: impl IntoIterator<Item = PathBuf>,
) -> Result<OsString> {
    let mut paths = Vec::new();
    for directory in directories {
        if !paths.contains(&directory) {
            paths.push(directory);
        }
    }
    for directory in TRUSTED_SYSTEM_TOOL_DIRS {
        // A platform may not provide every FHS directory, so absent system paths are skipped
        if let Ok(directory) = validate_tool_directory(Path::new(directory)) {
            if !paths.contains(&directory) {
                paths.push(directory);
            }
        }
    }
    std::env::join_paths(paths).map_err(|error| anyhow!("failed to build trusted PATH: {error}"))
}

fn validate_tool_directory(path: &Path) -> Result<PathBuf> {
    let canonical_path = fs::canonicalize(path).map_err(|error| {
        anyhow!(
            "failed to canonicalize tool directory {}: {error}",
            path.display()
        )
    })?;
    let metadata = fs::metadata(&canonical_path)?;
    if !metadata.is_dir() {
        return Err(anyhow!(
            "tool path parent is not a directory: {}",
            canonical_path.display()
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        // A writable group or other account could replace a tool after resolution
        if metadata.permissions().mode() & 0o022 != 0 || metadata.permissions().mode() & 0o111 == 0
        {
            return Err(anyhow!(
                "tool directory has unsafe permissions: {}",
                canonical_path.display()
            ));
        }

        // Only the current account or root may own a directory used for execution
        let uid = metadata.uid();
        let expected_uid = rustix::process::geteuid().as_raw();
        if uid != expected_uid && uid != 0 {
            return Err(anyhow!(
                "tool directory has an unexpected owner: {}",
                canonical_path.display()
            ));
        }
    }

    Ok(canonical_path)
}

fn validate_executable(path: &Path) -> Result<ValidatedExecutable> {
    if !path.is_absolute() {
        return Err(anyhow!("tool path is not absolute: {}", path.display()));
    }
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| anyhow!("failed to canonicalize tool {}: {error}", path.display()))?;
    if !is_acceptable_executable(&canonical_path) {
        return Err(anyhow!(
            "tool is not an acceptable executable: {}",
            path.display()
        ));
    }
    Ok(ValidatedExecutable {
        launch_path: path.to_path_buf(),
        canonical_path,
    })
}

#[cfg(unix)]
fn is_target_execution_variable(name: &OsStr) -> bool {
    let Some(target_setting) = name.as_bytes().strip_prefix(b"CARGO_TARGET_") else {
        return false;
    };

    [b"_LINKER".as_slice(), b"_RUNNER", b"_RUSTFLAGS"]
        .into_iter()
        .any(|suffix| {
            target_setting
                .strip_suffix(suffix)
                .is_some_and(|target| !target.is_empty())
        })
}

fn account_home_dir() -> Result<PathBuf> {
    #[cfg(unix)]
    {
        let uid = rustix::process::geteuid().as_raw() as libc::uid_t;
        let mut buffer = vec![0_u8; PASSWD_BUFFER_START];

        loop {
            let mut passwd = MaybeUninit::<libc::passwd>::zeroed();
            let mut result = ptr::null_mut();
            // SAFETY: every pointer targets live storage owned by this scope, the buffer is
            // writable for its full length, and libc writes the result pointer into result
            let status = unsafe {
                // getpwuid_r writes the passwd record and its strings into the caller buffer
                libc::getpwuid_r(
                    uid,
                    passwd.as_mut_ptr(),
                    buffer.as_mut_ptr().cast(),
                    buffer.len(),
                    &raw mut result,
                )
            };

            if status == 0 {
                if result.is_null() {
                    return Err(anyhow!("effective UID has no passwd entry"));
                }
                // SAFETY: libc returned success with a non-null result, so passwd was initialized
                let passwd = unsafe { passwd.assume_init() };
                if passwd.pw_dir.is_null() {
                    return Err(anyhow!("effective UID has no home directory"));
                }
                // SAFETY: pw_dir is a non-null NUL-terminated field owned by the live passwd
                // record and remains valid while the backing buffer stays in scope
                let home = unsafe { CStr::from_ptr(passwd.pw_dir).to_bytes().to_vec() };
                let home = PathBuf::from(OsString::from_vec(home));
                if !home.is_absolute() {
                    return Err(anyhow!("account home directory is not absolute"));
                }
                return Ok(home);
            }

            if status != libc::ERANGE {
                return Err(io::Error::from_raw_os_error(status).into());
            }
            let next_size = buffer
                .len()
                .checked_mul(2)
                .filter(|size| *size <= PASSWD_BUFFER_LIMIT)
                .ok_or_else(|| anyhow!("passwd entry exceeds the supported size limit"))?;
            buffer.resize(next_size, 0);
        }
    }

    #[cfg(not(unix))]
    {
        Err(anyhow!(
            "account home lookup is unsupported on this platform"
        ))
    }
}

fn is_acceptable_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        // Writable group or other bits would let another account replace the tool
        if metadata.permissions().mode() & 0o022 != 0 || metadata.permissions().mode() & 0o111 == 0
        {
            return false;
        }

        // User toolchains belong to the current account; system tools may belong to root
        let uid = metadata.uid();
        let expected_uid = rustix::process::geteuid().as_raw();
        uid == expected_uid || uid == 0
    }

    #[cfg(not(unix))]
    {
        true
    }
}

#[cfg(test)]
#[path = "tests/toolchain.rs"]
mod tests;
