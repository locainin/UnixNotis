//! CSS lint file loading and rule coordination

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use unixnotis_core::{build_modern_theme_custom_properties, Config};

use super::super::files::format_display_path;
use super::super::geometry::{collect_custom_property_scopes, CssCustomPropertyScopes};
use super::super::report::{CssCheckCategory, CssCheckDiagnostic};

#[cfg(test)]
#[path = "tests/runner.rs"]
mod tests;

#[derive(Debug)]
pub(in crate::css_check) struct CssCheckLintFinding {
    // Lint can point at the source when the scanner has a stable offset
    pub(in crate::css_check) line: Option<usize>,
    pub(in crate::css_check) column: Option<usize>,
    pub(in crate::css_check) message: String,
}

pub(in crate::css_check) fn lint_css_files(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
    config: &Config,
) -> Result<Vec<CssCheckDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut file_contents = Vec::new();
    for path in files {
        // Lint still reads every file directly because GTK parser callbacks do not cover
        // duplicate selectors, duplicate properties, or geometry-aware value hints
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read css file {}", path.display()))?;
        file_contents.push((path, contents));
    }

    // Modern tokens are generated at runtime, so lint needs the same token view even when
    // the physical css files only contain the consuming var() rules
    let generated_tokens = generated_theme_token_css(config);
    let combined_custom_properties = collect_custom_property_scopes(
        &std::iter::once(generated_tokens.as_str())
            .chain(file_contents.iter().map(|(_, contents)| contents.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    for (path, contents) in file_contents {
        let display_path = format_display_path(config_dir, display_root, path);
        // GTK only reports parser failures, so lint reads the raw file too
        let report = lint_css_contents_with_properties(&contents, &combined_custom_properties);
        for finding in report {
            diagnostics.push(CssCheckDiagnostic::warning_at(
                CssCheckCategory::Lint,
                display_path.clone(),
                finding.line,
                finding.column,
                finding.message,
            ));
        }
    }
    Ok(diagnostics)
}

pub(in crate::css_check::lint) fn lint_css_contents_with_properties(
    contents: &str,
    custom_properties: &CssCustomPropertyScopes,
) -> Vec<CssCheckLintFinding> {
    // Tests and file-based linting share one implementation so rule behavior does not drift
    super::scan::lint_css_contents_with_properties(contents, custom_properties)
}

fn generated_theme_token_css(config: &Config) -> String {
    build_modern_theme_custom_properties(&config.theme)
}
