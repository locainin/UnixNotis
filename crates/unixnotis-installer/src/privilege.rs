//! Installer privilege-boundary checks

use anyhow::{bail, Result};

pub(crate) fn reject_root_install(euid: u32) -> Result<()> {
    if euid == 0 {
        bail!("unixnotis-installer is user-level; do not run it as root or through sudo");
    }

    Ok(())
}

#[cfg(test)]
#[path = "tests/privilege.rs"]
mod tests;
