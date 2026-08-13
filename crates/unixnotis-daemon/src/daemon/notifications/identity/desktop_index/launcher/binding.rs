//! Package-launcher binding orchestration

use std::path::Path;

use super::super::super::executable::FileIdentity;
use super::super::model::PackageLauncherBinding;

/// Extract one literal runtime target without running or emulating the launcher
pub fn inspect_package_shell_launcher(
    path: &Path,
    expected_identity: FileIdentity,
) -> Option<PackageLauncherBinding> {
    // Reading through one no-follow descriptor binds syntax to the indexed file
    let launcher = super::read::read_launcher(path, expected_identity)?;
    let target_path = super::syntax::literal_final_exec_target(&launcher.contents)?;

    // The literal target must already be protected before package ownership is queried
    let target_identity = super::validation::protected_runtime_target(&target_path)?;
    Some(PackageLauncherBinding {
        launcher_path: path.to_path_buf(),
        launcher_identity: launcher.identity,
        launcher_digest: launcher.digest,
        target_path,
        target_identity,
    })
}

/// Reopen both files and repeat the literal-target proof before granting authority
pub fn launcher_binding_is_current(binding: &PackageLauncherBinding) -> bool {
    let Some(launcher) =
        super::read::read_launcher(&binding.launcher_path, binding.launcher_identity)
    else {
        return false;
    };
    if launcher.digest != binding.launcher_digest {
        return false;
    }
    if super::syntax::literal_final_exec_target(&launcher.contents).as_ref()
        != Some(&binding.target_path)
    {
        return false;
    }

    super::validation::protected_runtime_target(&binding.target_path)
        .is_some_and(|current| current.same_file(binding.target_identity))
}
