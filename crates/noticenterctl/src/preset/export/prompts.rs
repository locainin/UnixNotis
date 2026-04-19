//! Export warning and fix prompts
//!
//! Export uses these helpers to keep user-facing warnings out of the main flow

use anyhow::{anyhow, Result};
use std::io::IsTerminal;
use std::path::Path;
use unixnotis_core::Config;

use crate::preset::command_rules::{
    collect_host_specific_command_paths, rewrite_host_specific_command_paths,
    HostSpecificCommandPath,
};
use crate::preset::config_root::CollectedConfigFiles;
use crate::preset::css_asset_refs::{
    rewrite_host_specific_css_asset_refs_in_sources, ExternalCssAssetRef, HostSpecificCssAssetRef,
};
use crate::preset::export::checks::{
    capture_file_overrides, restore_file_overrides, rewrite_host_specific_script_paths_in_sources,
    HostSpecificScriptLeak,
};
use crate::preset::pathing::{confirm_continue_or_abort, prompt_yes_no};

pub(super) fn confirm_export_external_css_refs(
    external_refs: &[ExternalCssAssetRef],
) -> Result<()> {
    if external_refs.is_empty() {
        return Ok(());
    }

    // Print full context before asking to continue
    let details = format_external_css_ref_lines(external_refs);
    eprintln!(
        "preset export warning: found {} CSS asset reference(s) that leave the UnixNotis config directory or use remote URLs",
        external_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    confirm_continue_or_abort(
        "External CSS asset references were found; continue exporting anyway?",
        &format!(
            "preset export found CSS asset references that leave the UnixNotis config directory or use remote URLs; rerun interactively to confirm anyway\n{}",
            details.join("\n")
        ),
    )
}

pub(super) fn rewrite_host_specific_command_paths_if_requested<G>(
    config_dir: &Path,
    config: &mut Config,
    prompt_fix_host_specific_command_paths: G,
) -> Result<Vec<HostSpecificCommandPath>>
where
    G: FnOnce(&[HostSpecificCommandPath]) -> Result<bool>,
{
    let leaked_paths = collect_host_specific_command_paths(config_dir, config);
    if leaked_paths.is_empty() {
        return Ok(Vec::new());
    }

    // Show exact command slots so the choice is explicit
    let details = format_host_specific_command_path_lines(&leaked_paths);
    eprintln!(
        "preset export warning: found {} host-specific command path(s) under the UnixNotis config directory",
        leaked_paths.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    // Decline keeps current values and still exports
    if !prompt_fix_host_specific_command_paths(&leaked_paths)? {
        eprintln!(
            "preset export warning: leaving host-specific command paths unchanged in the bundle"
        );
        return Ok(Vec::new());
    }

    Ok(rewrite_host_specific_command_paths(config_dir, config))
}

pub(super) fn rewrite_host_specific_css_asset_refs_if_requested<G>(
    config_dir: &Path,
    collected: &mut CollectedConfigFiles,
    prompt_fix_host_specific_css_asset_refs: G,
) -> Result<Vec<HostSpecificCssAssetRef>>
where
    G: FnOnce(&[HostSpecificCssAssetRef]) -> Result<bool>,
{
    let snapshots = capture_file_overrides(&collected.files);

    // CSS rewrite happens in-memory only
    let leaked_refs =
        rewrite_host_specific_css_asset_refs_in_sources(config_dir, &mut collected.files)?;
    if leaked_refs.is_empty() {
        return Ok(Vec::new());
    }

    // Show exact url(...) values before keeping or dropping rewrites
    let details = format_host_specific_css_asset_ref_lines(&leaked_refs);
    eprintln!(
        "preset export warning: found {} host-specific CSS asset reference(s) under the UnixNotis config directory",
        leaked_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    match prompt_fix_host_specific_css_asset_refs(&leaked_refs) {
        Ok(true) => {
            // Keep staged rewrite bytes
            Ok(leaked_refs)
        }
        Ok(false) => {
            // Declining rewrite restores the exact staged file state from before the rewrite pass
            restore_file_overrides(&mut collected.files, &snapshots);
            eprintln!(
                "preset export warning: leaving host-specific CSS asset references unchanged in the bundle"
            );
            Ok(Vec::new())
        }
        Err(err) => {
            // Prompt failures must not leak half-rewritten staged bytes into later export logic
            restore_file_overrides(&mut collected.files, &snapshots);
            Err(err)
        }
    }
}

pub(super) fn rewrite_host_specific_script_paths_if_requested<G>(
    config_dir: &Path,
    collected: &mut CollectedConfigFiles,
    prompt_fix_host_specific_script_paths: G,
) -> Result<Vec<HostSpecificScriptLeak>>
where
    G: FnOnce(&[HostSpecificScriptLeak]) -> Result<bool>,
{
    let snapshots = capture_file_overrides(&collected.files);

    // Script rewrite also stays in-memory so live scripts are not touched
    let leaked_refs =
        rewrite_host_specific_script_paths_in_sources(config_dir, &mut collected.files)?;
    if leaked_refs.is_empty() {
        return Ok(Vec::new());
    }

    // Print one line per leak so prompt output is easy to scan
    let details = format_host_specific_script_path_lines(&leaked_refs);
    eprintln!(
        "preset export warning: found {} host-specific script path reference(s) under the UnixNotis config directory",
        leaked_refs.len()
    );
    for line in &details {
        eprintln!("{line}");
    }

    match prompt_fix_host_specific_script_paths(&leaked_refs) {
        Ok(true) => {
            // Keep staged script rewrites
            Ok(leaked_refs)
        }
        Ok(false) => {
            // Declining rewrite keeps the original script bytes and size in the staged archive
            restore_file_overrides(&mut collected.files, &snapshots);
            eprintln!(
                "preset export warning: leaving host-specific script path references unchanged in the bundle"
            );
            Ok(Vec::new())
        }
        Err(err) => {
            // Prompt failures must roll back staged rewrites too
            restore_file_overrides(&mut collected.files, &snapshots);
            Err(err)
        }
    }
}

pub(super) fn prompt_to_fix_host_specific_command_paths(
    _leaked_paths: &[HostSpecificCommandPath],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive mode can ask and continue in one run
        return prompt_yes_no(
            "Host-specific command paths were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    // Non-interactive mode cannot guess rewrite intent
    Err(anyhow!(
        "preset export found host-specific command paths under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

pub(super) fn prompt_to_fix_host_specific_css_asset_refs(
    _leaked_refs: &[HostSpecificCssAssetRef],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive mode can ask and continue in one run
        return prompt_yes_no(
            "Host-specific CSS asset references were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    Err(anyhow!(
        "preset export found host-specific CSS asset references under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
}

pub(super) fn prompt_to_fix_host_specific_script_paths(
    _leaked_refs: &[HostSpecificScriptLeak],
) -> Result<bool> {
    if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
        // Interactive mode can ask and continue in one run
        return prompt_yes_no(
            "Host-specific script paths were found; let noticenterctl rewrite them in the exported preset?",
        );
    }

    // Non-interactive mode cannot guess rewrite intent
    Err(anyhow!(
        "preset export found host-specific script path references under the UnixNotis config directory; rerun interactively to let noticenterctl rewrite them"
    ))
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
            // One warning line per found reference
            format!(
                "  - {} -> {} ({})",
                asset_ref.css_file.display(),
                asset_ref.asset_ref,
                detail
            )
        })
        .collect()
}

fn format_host_specific_command_path_lines(
    leaked_paths: &[HostSpecificCommandPath],
) -> Vec<String> {
    leaked_paths
        .iter()
        .map(|leak| {
            // Show exact slot and command for quick review
            format!(
                "  - {} = {} (absolute path under the config root; let noticenterctl rewrite it to a config-root-relative command)",
                leak.slot, leak.command
            )
        })
        .collect()
}

fn format_host_specific_css_asset_ref_lines(
    leaked_refs: &[HostSpecificCssAssetRef],
) -> Vec<String> {
    leaked_refs
        .iter()
        .map(|leak| {
            // Include target rewrite so prompt answer is clear
            format!(
                "  - {} -> {} (host-local config path; let noticenterctl rewrite it to {})",
                leak.css_file.display(),
                leak.asset_ref,
                leak.rewritten_ref
            )
        })
        .collect()
}

fn format_host_specific_script_path_lines(leaked_refs: &[HostSpecificScriptLeak]) -> Vec<String> {
    leaked_refs
        .iter()
        .map(|leak| {
            let matched = leak.needles.join(", ");
            // Include replacement text to show final form
            format!(
                "  - {} contains {} (let noticenterctl rewrite it to {})",
                leak.script_path.display(),
                matched,
                leak.rewritten_to
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        rewrite_host_specific_css_asset_refs_if_requested,
        rewrite_host_specific_script_paths_if_requested,
    };
    use crate::preset::config_root::CollectedConfigFiles;
    use crate::preset::config_root::PresetFileSource;
    use anyhow::anyhow;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new(name: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock moved backwards")
                .as_nanos();
            let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("unixnotis-export-prompts-{name}-{stamp}-{serial}"));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn write(&self, relative_path: &str, contents: &[u8]) -> PresetFileSource {
            let source_path = self.path.join(relative_path);
            if let Some(parent) = source_path.parent() {
                fs::create_dir_all(parent).expect("create parent dirs");
            }
            fs::write(&source_path, contents).expect("write file");
            let metadata = fs::metadata(&source_path).expect("metadata");
            #[cfg(unix)]
            let mode = metadata.permissions().mode() & 0o777;
            #[cfg(not(unix))]
            let mode = 0o644;

            PresetFileSource {
                relative_path: PathBuf::from(relative_path),
                source_path,
                size: metadata.len(),
                mode,
                contents_override: None,
            }
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn css_prompt_error_restores_staged_bytes_and_size() {
        let root = TempDirGuard::new("css-restore");
        let asset_path = root.path.join("assets/example.png");
        fs::create_dir_all(asset_path.parent().expect("asset parent")).expect("create asset dir");
        fs::write(&asset_path, b"png").expect("write asset");
        let original = format!(
            ".panel {{ background-image: url(\"file://{}\"); }}\n",
            asset_path.display()
        );
        let file = root.write("base.css", original.as_bytes());
        let original_size = file.size;
        let mut collected = CollectedConfigFiles {
            files: vec![file],
            ..CollectedConfigFiles::default()
        };

        let error = rewrite_host_specific_css_asset_refs_if_requested(
            &root.path,
            &mut collected,
            |_leaks| Err(anyhow!("prompt failed")),
        )
        .expect_err("prompt should fail");

        assert!(error.to_string().contains("prompt failed"));
        assert!(collected.files[0].contents_override.is_none());
        assert_eq!(collected.files[0].size, original_size);
    }

    #[test]
    fn script_prompt_error_restores_staged_bytes_and_size() {
        let root = TempDirGuard::new("script-restore");
        let original = format!(
            "#!/bin/sh\necho \"{}/assets/example.png\"\n",
            root.path.display()
        );
        let file = root.write("scripts/demo-widget", original.as_bytes());
        let original_size = file.size;
        let mut collected = CollectedConfigFiles {
            files: vec![file],
            ..CollectedConfigFiles::default()
        };

        let error =
            rewrite_host_specific_script_paths_if_requested(&root.path, &mut collected, |_leaks| {
                Err(anyhow!("prompt failed"))
            })
            .expect_err("prompt should fail");

        assert!(error.to_string().contains("prompt failed"));
        assert!(collected.files[0].contents_override.is_none());
        assert_eq!(collected.files[0].size, original_size);
    }
}
