//! Package-provider discovery, batching, and output parsing

use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Command;

use super::super::super::executable::executable_evidence_for_path;
use super::cache::OwnershipLookup;
use super::process::run_package_query;
use super::rpm::query_rpm_ownership;
use super::{InstallProvenance, PackageProvider};

pub(super) const MAX_COMMAND_ARGUMENT_BYTES: usize = 192 * 1024;
pub(super) const MAX_COMMAND_PATHS: usize = 4_096;
const MAX_OWNERSHIP_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const MAX_PACKAGE_ID_BYTES: usize = 256;

#[derive(Debug, Clone)]
pub(super) struct PackageProviderCommand {
    pub(super) provider: PackageProvider,
    pub(super) executable: PathBuf,
}

pub(super) fn detect_package_provider() -> Option<PackageProviderCommand> {
    [
        ("pacman", PackageProvider::Pacman),
        ("dpkg-query", PackageProvider::Dpkg),
        ("rpm", PackageProvider::Rpm),
    ]
    .into_iter()
    .find_map(|(program, provider)| {
        let executable = unixnotis_core::util::trusted_system_program_path(program)?;
        let evidence = executable_evidence_for_path(&executable)?;
        // Provider output affects attribution, so user-writable commands are never accepted
        (evidence.identity.is_system_managed() && evidence.identity.is_executable_regular())
            .then_some(PackageProviderCommand {
                provider,
                executable: evidence.canonical_path,
            })
    })
}

pub(super) fn query_package_ownership(
    provider: &PackageProviderCommand,
    paths: &[PathBuf],
) -> HashMap<PathBuf, OwnershipLookup> {
    match provider.provider {
        PackageProvider::Pacman => query_in_chunks(provider, paths, &["-Qo"], parse_pacman_output),
        PackageProvider::Dpkg => query_in_chunks(provider, paths, &["--search"], parse_dpkg_output),
        // RPM output does not retain each selector, so bounded workers query paths separately
        PackageProvider::Rpm => query_rpm_ownership(provider, paths),
    }
}

fn query_in_chunks(
    provider: &PackageProviderCommand,
    paths: &[PathBuf],
    arguments: &[&str],
    parser: OwnershipOutputParser,
) -> HashMap<PathBuf, OwnershipLookup> {
    let mut resolved = HashMap::new();
    let mut remaining = paths;
    while remaining.split_first().is_some() {
        // A one-path floor preserves progress even if a future chunk policy returns zero
        let chunk_len = ownership_chunk_len(remaining).max(1).min(remaining.len());
        let (chunk, next) = remaining.split_at(chunk_len);
        let mut command = Command::new(&provider.executable);
        command
            .args(arguments)
            .args(chunk)
            .env_clear()
            .env("LC_ALL", "C");
        match run_package_query(&mut command, MAX_OWNERSHIP_OUTPUT_BYTES) {
            Ok(output) => {
                let parsed = parser(&output.stdout, chunk, provider.provider);
                for path in chunk {
                    let lookup = parsed.get(path).cloned().map_or_else(
                        || {
                            if output.status.success() && output.stdout.is_empty() {
                                OwnershipLookup::Negative(super::cache::NegativeCause::NotOwned)
                            } else if output.status.success() {
                                OwnershipLookup::Negative(
                                    super::cache::NegativeCause::MalformedOutput,
                                )
                            } else {
                                OwnershipLookup::Negative(
                                    super::cache::NegativeCause::ProviderFailure,
                                )
                            }
                        },
                        OwnershipLookup::Known,
                    );
                    resolved.insert(path.clone(), lookup);
                }
            }
            Err(error) => {
                let cause = error.negative_cause();
                resolved.extend(
                    chunk
                        .iter()
                        .cloned()
                        .map(|path| (path, OwnershipLookup::Negative(cause))),
                );
            }
        }
        remaining = next;
    }
    resolved
}

pub(super) fn ownership_chunk_len(paths: &[PathBuf]) -> usize {
    let mut bytes = 0_usize;
    let mut end = 0_usize;
    while end < paths.len() && end < MAX_COMMAND_PATHS {
        let next = paths[end].as_os_str().as_bytes().len().saturating_add(1);
        // The first path always advances so even an oversized selector cannot stall the scan
        if end > 0 && bytes.saturating_add(next) > MAX_COMMAND_ARGUMENT_BYTES {
            break;
        }
        bytes = bytes.saturating_add(next);
        end = end.saturating_add(1);
    }
    end
}

type OwnershipOutputParser =
    fn(&[u8], &[PathBuf], PackageProvider) -> HashMap<PathBuf, InstallProvenance>;

pub(super) fn parse_pacman_output(
    output: &[u8],
    paths: &[PathBuf],
    provider: PackageProvider,
) -> HashMap<PathBuf, InstallProvenance> {
    let expected = paths
        .iter()
        .map(|path| (path.as_os_str().as_bytes(), path))
        .collect::<HashMap<_, _>>();
    output
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let marker = b" is owned by ";
            let position = line
                .windows(marker.len())
                .position(|window| window == marker)?;
            let path = expected.get(&line[..position])?;
            let package = line.get(position.saturating_add(marker.len())..)?;
            let package = package.split(|byte| *byte == b' ').next()?;
            package_provenance(provider, package).map(|owner| ((*path).clone(), owner))
        })
        .collect()
}

pub(super) fn parse_dpkg_output(
    output: &[u8],
    paths: &[PathBuf],
    provider: PackageProvider,
) -> HashMap<PathBuf, InstallProvenance> {
    let expected = paths
        .iter()
        .map(|path| (path.as_os_str().as_bytes(), path))
        .collect::<HashMap<_, _>>();
    output
        .split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let position = line.windows(2).rposition(|window| window == b": ")?;
            let package = line.get(..position)?.split(|byte| *byte == b',').next()?;
            let path = expected.get(line.get(position.saturating_add(2)..)?)?;
            package_provenance(provider, package).map(|owner| ((*path).clone(), owner))
        })
        .collect()
}

pub(super) fn package_provenance(
    provider: PackageProvider,
    package: &[u8],
) -> Option<InstallProvenance> {
    if package.is_empty()
        || package.len() > MAX_PACKAGE_ID_BYTES
        || package
            .iter()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return None;
    }
    Some(InstallProvenance::Package {
        provider,
        package_id: std::str::from_utf8(package).ok()?.to_string(),
    })
}
