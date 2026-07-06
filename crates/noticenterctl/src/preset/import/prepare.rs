//! Bundle validation and write-plan preparation

use std::path::Path;

use anyhow::{anyhow, Context, Result};

use super::super::archive::read_bundle;
use super::super::css_asset_refs::{
    collect_external_css_asset_refs_from_bundle, ExternalCssAssetRef,
};
use super::super::filesystem::ensure_no_symlink_ancestors;
use super::super::pathing::{
    parse_except_paths, relative_path_matches_exclusion, validate_preset_bundle_path,
};
use super::checks::{
    collect_imported_exec_content, validate_imported_command_paths_stay_in_root,
    validate_imported_theme_paths_stay_in_root, ImportedExecContent,
};
use super::plan::{build_import_plan, ImportPlan};

pub(super) struct PreparedImport {
    // The write plan is reused by dry-run, the test helper, and the CLI import path
    pub(super) plan: ImportPlan,
}

pub(super) fn prepare_import(
    config_dir: &Path,
    input_path: &Path,
    except: &[String],
    allow_exec: bool,
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
    let bundle = read_bundle(input_path).context("read preset bundle for import")?;

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
        let local_config_path = config_dir.join("config.toml");
        // Keeping the local config means its theme paths still control the later css-check setup
        std::fs::read(&local_config_path).with_context(|| {
            format!(
                "read existing config.toml kept by --except from {}",
                local_config_path.display()
            )
        })?
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
    // Shared presets default to data-only imports unless the caller explicitly trusts exec content
    let exec_content =
        collect_imported_exec_content(&effective_config_bytes, &included_bundle_files)?;
    // The exec review prompt must run before the CSS prompt so trust comes first
    confirm_exec_content(&exec_content, allow_exec)?;
    // CSS asset refs are warning-only, but the prompt still needs to happen before any write starts
    let external_css_refs =
        collect_external_css_asset_refs_from_bundle(config_dir, &included_bundle_files);
    confirm_external_css_refs(&external_css_refs)?;
    // The write plan is built last so prompts cannot leave behind partial staging state
    let plan = build_import_plan(config_dir, bundle.files, &exclusions)?;
    Ok(PreparedImport { plan })
}
