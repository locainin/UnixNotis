//! CLI-facing preset import runner

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use crate::css_check::run as run_css_check;

use super::super::super::pathing::resolve_cli_bundle_path;
use super::super::review::exec_review::confirm_import_exec_content;
use super::super::review::prompts::confirm_import_external_css_refs;
use super::super::transaction::commit::commit_import_plan;
use super::super::transaction::prepare::prepare_import;
use super::summary::{build_summary, print_summary};

pub(in crate::preset) fn run_import(
    input_path: &Path,
    except: &[String],
    dry_run: bool,
    allow_exec: bool,
    allow_external_css: bool,
) -> Result<()> {
    // Resolve the live config root once for the CLI path
    let config_dir = Config::default_config_dir().context("resolve config directory")?;
    // CLI import accepts a missing extension and can append it after confirmation
    let input_path = resolve_cli_bundle_path(input_path)?;
    let prepared = prepare_import(
        &config_dir,
        &input_path,
        except,
        super::super::transaction::prepare::ImportTrustPolicy {
            allow_exec,
            allow_external_css,
        },
        confirm_import_external_css_refs,
        confirm_import_exec_content,
    )?;

    if dry_run {
        let summary = build_summary(&prepared.plan, None, true);
        print_summary(&summary)?;
        return Ok(());
    }

    let (backup_dir, css_check_result) = commit_import_plan(&config_dir, &prepared.plan, || {
        post_import_css_check(&config_dir)
    })?;
    let summary = build_summary(&prepared.plan, backup_dir, false);
    print_summary(&summary)?;

    if let Err(err) = css_check_result {
        // The import committed, but the shared theme still has CSS problems the user should see
        return Err(anyhow!(
            "preset import completed, but css-check failed after import: {err}"
        ));
    }

    Ok(())
}

pub(in crate::preset) fn post_import_css_check(config_dir: &Path) -> Result<()> {
    post_import_css_check_with(config_dir, run_css_check)
}

pub(in crate::preset) fn post_import_css_check_with<F>(
    config_dir: &Path,
    run_check: F,
) -> Result<()>
where
    F: FnOnce(Option<std::path::PathBuf>) -> Result<()>,
{
    // Import always writes this exact file, so environment overrides must not redirect validation
    run_check(Some(config_dir.join("config.toml")))
}
