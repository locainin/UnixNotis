//! Bundle validation and write-plan preparation

use std::path::Path;

use anyhow::{anyhow, Context, Result};

use super::super::super::archive::{read_bundle, MAX_PRESET_FILE_BYTES};
use super::super::super::css_asset_refs::{
    collect_external_css_asset_refs_from_bundle, ExternalCssAssetRef,
};
use super::super::super::filesystem::{
    ensure_no_symlink_ancestors, open_secure_dir_all, read_relative_file_secure_bounded,
};
use super::super::super::pathing::{
    parse_except_paths, relative_path_matches_exclusion, validate_preset_bundle_path,
};
use super::super::css_assets::harden_imported_css_assets;
use super::super::review::checks::{
    collect_imported_exec_content, validate_imported_command_paths_stay_in_root,
    validate_imported_icon_assets, validate_imported_theme_paths_stay_in_root, ImportedExecContent,
};
use super::plan::{build_import_plan, ImportPlan};

pub(in crate::preset) struct PreparedImport {
    // The write plan is reused by dry-run, the test helper, and the CLI import path
    pub(in crate::preset) plan: ImportPlan,
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::preset) struct ImportTrustPolicy {
    pub(in crate::preset) allow_exec: bool,
    pub(in crate::preset) allow_external_css: bool,
}

pub(in crate::preset) fn prepare_import(
    config_dir: &Path,
    input_path: &Path,
    except: &[String],
    trust_policy: ImportTrustPolicy,
    confirm_external_css_refs: impl FnOnce(&[ExternalCssAssetRef]) -> Result<()>,
    confirm_exec_content: impl FnOnce(&ImportedExecContent, bool) -> Result<()>,
) -> Result<PreparedImport> {
    validate_preset_bundle_path(input_path)?;
    // The whole config-root path must be free of symlink hops before any write plan is built
    ensure_no_symlink_ancestors(config_dir)?;

    let exclusions = parse_except_paths(except)?;
    // A kept-local config.toml means the bundle config never drives post-import theme setup
    let imports_config_toml =
        !relative_path_matches_exclusion(Path::new("config.toml"), &exclusions);
    // Read and validate the full bundle before touching the local config tree
    let mut bundle = read_bundle(input_path).context("read preset bundle for import")?;

    if !bundle
        .files
        .iter()
        .any(|file| file.relative_path == Path::new("config.toml"))
    {
        // Import depends on one config source of truth, so bundles without config.toml are invalid
        return Err(anyhow!(
            "preset bundle is missing config.toml and cannot be imported"
        ));
    }

    // Import should validate the config that will actually drive post-import theme setup
    let effective_config_bytes = if imports_config_toml {
        let bundled_config = bundle
            .files
            .iter()
            // Reuse the already validated bundle payload instead of reading from disk again
            .find(|file| file.relative_path == Path::new("config.toml"))
            .ok_or_else(|| {
                anyhow!("preset bundle is missing config.toml and cannot be imported")
            })?;
        bundled_config.contents.clone()
    } else {
        let config_root_fd = open_secure_dir_all(config_dir)
            .with_context(|| format!("open config directory {}", config_dir.display()))?;
        // Keeping local config still requires a contained descriptor read before it drives review
        read_relative_file_secure_bounded(
            &config_root_fd,
            Path::new("config.toml"),
            MAX_PRESET_FILE_BYTES,
        )
        .context("securely read existing config.toml kept by --except")?
        .0
    };

    let included_bundle_files = bundle
        .files
        .iter()
        // Warning and review prompts should only talk about files that will actually be applied
        .filter(|file| !relative_path_matches_exclusion(&file.relative_path, &exclusions))
        .cloned()
        .collect::<Vec<_>>();

    // This closes both bundled and kept-local config chains before any file is written
    validate_imported_theme_paths_stay_in_root(config_dir, &effective_config_bytes)?;
    // Explicit path commands should stay inside the shared config root too
    validate_imported_command_paths_stay_in_root(config_dir, &effective_config_bytes)?;
    // Widget image assets must stay config-relative even though missing optional files can fall back at runtime
    validate_imported_icon_assets(&effective_config_bytes, &included_bundle_files)?;
    // Shared presets default to data-only imports unless the caller explicitly trusts exec content
    let exec_content =
        collect_imported_exec_content(&effective_config_bytes, &included_bundle_files)?;
    // The exec review prompt must run before the CSS prompt so trust comes first
    confirm_exec_content(&exec_content, trust_policy.allow_exec)?;
    // Release the first validation snapshot before image decoding allocates bounded pixel buffers
    drop(exec_content);
    drop(included_bundle_files);
    // Local and embedded images become bounded PNG files before GTK can inspect their source bytes
    harden_imported_css_assets(config_dir, &mut bundle.files, &exclusions)?;
    let included_bundle_files = bundle
        .files
        .iter()
        .filter(|file| !relative_path_matches_exclusion(&file.relative_path, &exclusions))
        .cloned()
        .collect::<Vec<_>>();
    // External references need a deliberate expert flag in addition to any terminal confirmation
    let external_css_refs =
        collect_external_css_asset_refs_from_bundle(config_dir, &included_bundle_files)?;
    if !external_css_refs.is_empty() && !trust_policy.allow_external_css {
        let details =
            super::super::review::prompts::format_external_css_ref_lines(&external_css_refs);
        return Err(anyhow!(
            "preset import found CSS asset references that leave the UnixNotis config directory or use remote URLs; rerun with --allow-external-css only if those references are trusted\n{}",
            details.join("\n")
        ));
    }
    confirm_external_css_refs(&external_css_refs)?;
    // The write plan is built last so prompts cannot leave behind partial staging state
    let plan = build_import_plan(config_dir, bundle.files, &exclusions)?;
    Ok(PreparedImport { plan })
}
