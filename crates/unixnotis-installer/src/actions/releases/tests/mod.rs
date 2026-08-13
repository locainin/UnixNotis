mod entrypoints;
mod health;
mod journal;
mod manifest;
mod recovery;
mod transaction;

use std::path::Path;

use anyhow::Result;

use super::transaction::install_release_generation_transaction;
use crate::paths::InstallPaths;

pub(super) fn install_release_generation<F, R, G>(
    paths: &InstallPaths,
    release_source: &Path,
    binaries: &[String],
    precommit: F,
    reserve_activation: R,
) -> Result<String>
where
    F: FnMut() -> Result<()>,
    R: FnMut() -> Result<G>,
{
    install_release_generation_transaction(
        paths,
        release_source,
        binaries,
        precommit,
        reserve_activation,
        || Ok(()),
    )
}

pub(super) fn install_release_generation_with_reservation_check<F, R, G, C>(
    paths: &InstallPaths,
    release_source: &Path,
    binaries: &[String],
    precommit: F,
    reserve_activation: R,
    reserved_check: C,
) -> Result<String>
where
    F: FnMut() -> Result<()>,
    R: FnMut() -> Result<G>,
    C: FnMut() -> Result<()>,
{
    install_release_generation_transaction(
        paths,
        release_source,
        binaries,
        precommit,
        reserve_activation,
        reserved_check,
    )
}
