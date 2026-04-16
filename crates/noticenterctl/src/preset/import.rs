//! Preset import flow for applying a bundle into the live config tree
//!
//! Import validates the bundle first, builds a write plan, optionally reports it,
//! then commits the final backup snapshot only after the staged import is ready to finish

#[path = "import/apply.rs"]
mod apply;
#[path = "import/checks.rs"]
mod checks;
#[path = "import/exec_review.rs"]
mod exec_review;
#[path = "import/plan.rs"]
mod plan;

use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use unixnotis_core::Config;

use crate::main_css_check::run_css_check;

use self::apply::{apply_import_plan, finalize_import_transaction, rollback_import_transaction};
use self::checks::{
    collect_imported_exec_content, validate_config_command_paths_for_import,
    validate_config_theme_paths_stay_in_root, validate_imported_command_paths_stay_in_root,
    validate_imported_theme_paths_stay_in_root, ImportedExecContent,
};
use self::exec_review::confirm_import_exec_content;
use self::plan::{build_import_plan, ImportPlan};
use super::archive::read_bundle;
use super::css_asset_refs::{collect_external_css_asset_refs_from_bundle, ExternalCssAssetRef};
use super::filesystem::ensure_no_symlink_ancestors;
use super::pathing::{
    confirm_continue_or_abort, parse_except_paths, relative_path_matches_exclusion,
    resolve_cli_bundle_path, validate_preset_bundle_path,
};

#[derive(Debug)]
pub(super) struct ImportSummary {
    // Number of files that will be or were applied from the bundle
    pub(super) file_count: usize,
    // Files that did not exist locally before import
    pub(super) created: usize,
    // Files that already existed and needed a backup first
    pub(super) overwritten: usize,
    // Bundle files intentionally left untouched because of --except
    pub(super) excluded: usize,
    // Backup directory is present only when an overwrite happened
    pub(super) backup_dir: Option<PathBuf>,
    // Dry-run keeps the same output shape without touching the filesystem
    pub(super) dry_run: bool,
}

struct PreparedImport {
    // The write plan is reused by dry-run, the test helper, and the CLI import path
    plan: ImportPlan,
}

pub(super) fn run_import(
    input_path: &Path,
    except: &[String],
    dry_run: bool,
    allow_exec: bool,
) -> Result<()> {
    // Resolve the live config root once for the CLI path
    let config_dir = Config::default_config_dir().context("resolve config directory")?;
    // CLI import accepts a missing extension and can append it after confirmation
    let input_path = resolve_cli_bundle_path(input_path)?;
    let prepared = prepare_import(
        &config_dir,
        &input_path,
        except,
        allow_exec,
        confirm_import_external_css_refs,
        confirm_import_exec_content,
    )?;

    if dry_run {
        let summary = build_summary(&prepared.plan, None, true);
        print_summary(&summary);
        return Ok(());
    }

    // Apply first, then keep the transaction open until the post-import checks finish
    let transaction = apply_import_plan(&config_dir, &prepared.plan)?;

    // Reload the active config after import so css-check validates the setup that was just applied
    let config_path = config_dir.join("config.toml");
    let config = match Config::load_from_path(&config_path)
        .context("load imported config.toml before css-check")
    {
        Ok(config) => config,
        Err(err) => {
            rollback_import_transaction(transaction)?;
            return Err(err);
        }
    };
    // Recheck the live config so `--except config.toml` cannot reuse an outside local theme path
    if let Err(err) = validate_config_theme_paths_stay_in_root(&config_dir, &config) {
        rollback_import_transaction(transaction)?;
        return Err(err);
    }
    if let Err(err) = validate_config_command_paths_for_import(&config_dir, &config) {
        rollback_import_transaction(transaction)?;
        return Err(err);
    }

    // Imported presets should be checked right away so broken shared CSS is obvious
    println!("preset import check: running css-check on imported theme files");
    let css_check_result = run_css_check();
    let backup_dir = finalize_import_transaction(transaction)?;
    let summary = build_summary(&prepared.plan, backup_dir, false);
    print_summary(&summary);

    if let Err(err) = css_check_result {
        // The import committed, but the shared theme still has CSS problems the user should see
        return Err(anyhow!(
            "preset import completed, but css-check failed after import: {err}"
        ));
    }

    Ok(())
}

#[cfg(test)]
pub(super) fn import_preset_into(
    config_dir: &Path,
    input_path: &Path,
    except: &[String],
    dry_run: bool,
) -> Result<ImportSummary> {
    // The shared helper keeps tests off stdin while the real CLI still uses the prompt path
    import_preset_into_with_confirm(
        config_dir,
        input_path,
        except,
        dry_run,
        false,
        confirm_import_external_css_refs,
        confirm_import_exec_content,
    )
}

#[cfg(test)]
pub(super) fn import_preset_into_with_confirm<F, G>(
    config_dir: &Path,
    input_path: &Path,
    except: &[String],
    dry_run: bool,
    allow_exec: bool,
    confirm_external_css_refs: F,
    confirm_exec_content: G,
) -> Result<ImportSummary>
where
    F: FnOnce(&[ExternalCssAssetRef]) -> Result<()>,
    G: FnOnce(&ImportedExecContent, bool) -> Result<()>,
{
    // Tests inject a fixed answer here so the import plan can be checked without terminal prompts
    let prepared = prepare_import(
        config_dir,
        input_path,
        except,
        allow_exec,
        confirm_external_css_refs,
        confirm_exec_content,
    )?;

    if dry_run {
        // Dry-run reports the exact write plan without creating backups or files
        return Ok(build_summary(&prepared.plan, None, true));
    }

    // Test helpers do not run css-check, but they still use the same staged apply and commit flow
    let transaction = apply_import_plan(config_dir, &prepared.plan)?;
    let backup_dir = finalize_import_transaction(transaction)?;
    Ok(build_summary(&prepared.plan, backup_dir, false))
}

fn prepare_import(
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

fn build_summary(plan: &ImportPlan, backup_dir: Option<PathBuf>, dry_run: bool) -> ImportSummary {
    ImportSummary {
        file_count: plan.items.len(),
        created: plan.created,
        overwritten: plan.overwritten,
        excluded: plan.excluded,
        backup_dir,
        dry_run,
    }
}

fn print_summary(summary: &ImportSummary) {
    println!(
        "preset import {}: {} file(s), {} created, {} overwritten, {} excluded",
        if summary.dry_run { "dry-run ok" } else { "ok" },
        summary.file_count,
        summary.created,
        summary.overwritten,
        summary.excluded
    );
    if let Some(backup_dir) = summary.backup_dir.as_ref() {
        println!("preset import backup: {}", backup_dir.display());
    }
}

fn confirm_import_external_css_refs(external_refs: &[ExternalCssAssetRef]) -> Result<()> {
    if external_refs.is_empty() {
        return Ok(());
    }

    // The caller needs the concrete file and ref before deciding whether portability matters here
    let details = format_external_css_ref_lines(external_refs);
    eprintln!(
        "preset import warning: found {} CSS asset reference(s) that leave the UnixNotis config directory or use remote URLs",
        external_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    confirm_continue_or_abort(
        "External CSS asset references were found; continue importing anyway?",
        &format!(
            "preset import found CSS asset references that leave the UnixNotis config directory or use remote URLs; rerun interactively to confirm anyway\n{}",
            details.join("\n")
        ),
    )
}

fn format_external_css_ref_lines(external_refs: &[ExternalCssAssetRef]) -> Vec<String> {
    external_refs
        .iter()
        .map(|asset_ref| {
            let detail = if asset_ref.reason == "remote url" {
                "remote URL".to_string()
            } else {
                asset_ref.reason.clone()
            };
            // One-line rows make the warning easy to scan before the prompt is shown
            format!(
                "  - {} -> {} ({})",
                asset_ref.css_file.display(),
                asset_ref.asset_ref,
                detail
            )
        })
        .collect()
}

#[cfg(test)]
#[path = "import/tests.rs"]
mod tests;
