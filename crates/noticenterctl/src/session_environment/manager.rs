//! Explicit and artifact-backed service-manager selection

use std::fs;

use anyhow::{bail, Context, Result};
use unixnotis_core::service_manager::{
    resolve_service_manager_paths, ServiceManagerKind, ServiceManagerPaths,
};

use crate::cli::DoctorServiceManagerArg;

pub(super) fn select_manager(requested: DoctorServiceManagerArg) -> Result<ServiceManagerPaths> {
    // Explicit CLI choices bypass artifact probing and ambiguity checks
    let kind = match requested {
        DoctorServiceManagerArg::Auto => detect_installed_manager()?,
        DoctorServiceManagerArg::Systemd => ServiceManagerKind::Systemd,
        DoctorServiceManagerArg::Dinit => ServiceManagerKind::Dinit,
        DoctorServiceManagerArg::Runit => ServiceManagerKind::Runit,
        DoctorServiceManagerArg::S6 => ServiceManagerKind::S6,
        DoctorServiceManagerArg::Manual => {
            bail!("manual launches do not have a service environment to synchronize")
        }
    };
    resolve_service_manager_paths(kind).context("resolve service-manager paths")
}

fn detect_installed_manager() -> Result<ServiceManagerKind> {
    // Only installer-owned artifacts count as evidence for automatic selection
    let installed = ServiceManagerKind::all()
        .into_iter()
        .filter(|kind| {
            resolve_service_manager_paths(*kind).is_ok_and(|paths| manager_artifact_exists(&paths))
        })
        .collect::<Vec<_>>();
    select_detected_manager(&installed)
}

pub(super) fn select_detected_manager(
    installed: &[ServiceManagerKind],
) -> Result<ServiceManagerKind> {
    // Automatic mode is safe only when one installed backend is unambiguous
    match installed {
        [kind] => Ok(*kind),
        [] => bail!(
            "no installed UnixNotis user service was found; pass --service-manager explicitly"
        ),
        _ => {
            bail!("multiple UnixNotis user services were found; pass --service-manager explicitly")
        }
    }
}

pub(super) fn manager_artifact_exists(paths: &ServiceManagerPaths) -> bool {
    // Every manager stores its primary daemon artifact at a stable relative path
    let artifact = match paths.kind {
        ServiceManagerKind::Systemd => paths.artifact_root.join("unixnotis-daemon.service"),
        ServiceManagerKind::Dinit => paths.artifact_root.join("unixnotis-daemon"),
        ServiceManagerKind::Runit => paths.artifact_root.join("unixnotis-daemon"),
        ServiceManagerKind::S6 => paths.artifact_root.join("sv").join("unixnotis-daemon"),
    };
    // Symlink metadata avoids following an attacker-controlled artifact target
    fs::symlink_metadata(artifact)
        .is_ok_and(|metadata| metadata.file_type().is_file() || metadata.file_type().is_dir())
}
