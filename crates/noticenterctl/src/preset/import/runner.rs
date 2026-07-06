//! CLI-facing preset import runner

use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::Config;

use crate::css_check::run as run_css_check;

use super::super::pathing::resolve_cli_bundle_path;
use super::commit::commit_import_plan;
use super::exec_review::confirm_import_exec_content;
use super::prepare::prepare_import;
use super::prompts::confirm_import_external_css_refs;
use super::summary::{build_summary, print_summary};

pub(in crate::preset) fn run_import(
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

    let (backup_dir, css_check_result) =
        commit_import_plan(&config_dir, &prepared.plan, run_css_check)?;
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
