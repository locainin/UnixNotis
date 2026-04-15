//! Lint rules for css-check

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use unixnotis_core::{build_modern_theme_custom_properties, gtk_css_features_for_version, Config};

use super::main_css_check_files::format_display_path;
use super::main_css_check_geometry::{
    can_model_horizontal_size_value, collect_custom_property_scopes, CssCustomPropertyScopes,
};
use super::main_css_check_parse::{
    next_css_block_with_offsets, normalize_selector, parse_css_declarations_with_offsets,
    should_recurse_at_rule, split_selectors, strip_css_comments,
};
use super::main_css_check_policy::is_horizontal_size_property;
use super::main_css_check_report::{CssCheckCategory, CssCheckDiagnostic};

#[derive(Debug)]
pub(super) struct CssCheckLintFinding {
    pub(super) code: &'static str,
    // Lint can point at the source when the scanner has a stable offset
    pub(super) line: Option<usize>,
    pub(super) column: Option<usize>,
    pub(super) message: String,
}

pub(super) fn lint_css_files(
    files: &[PathBuf],
    config_dir: &Path,
    display_root: &str,
) -> Result<Vec<CssCheckDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut file_contents = Vec::new();
    for path in files {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read css file {}", path.display()))?;
        file_contents.push((path, contents));
    }

    // Modern tokens are generated at runtime, so lint needs the same token view even when
    // the physical css files only contain the consuming var() rules
    let generated_tokens = generated_theme_token_css().unwrap_or_default();
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
                finding.code,
                display_path.clone(),
                finding.line,
                finding.column,
                finding.message,
            ));
        }
    }
    Ok(diagnostics)
}

#[cfg(test)]
pub(super) fn lint_css_contents(contents: &str) -> Vec<CssCheckLintFinding> {
    lint_css_contents_with_properties(contents, &collect_custom_property_scopes(contents))
}

fn lint_css_contents_with_properties(
    contents: &str,
    custom_properties: &CssCustomPropertyScopes,
) -> Vec<CssCheckLintFinding> {
    let mut warnings = Vec::new();

    // Strip comments first so block scanning stays honest
    let stripped = strip_css_comments(contents);

    // Repeated color names usually mean an accidental override
    let mut color_defs: HashMap<String, usize> = HashMap::new();
    let mut offset = 0usize;
    for segment in stripped.split_inclusive('\n') {
        // Running offsets stay correct even when the same line text appears more than once
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("@define-color") {
            if let Some(name) = rest.split_whitespace().next() {
                let count = color_defs.entry(name.to_string()).or_insert(0);
                *count += 1;
                if *count > 1 {
                    let trimmed_start = line.len().saturating_sub(trimmed.len());
                    let (lint_line, lint_column) =
                        line_column_for_offset(&stripped, offset + trimmed_start);
                    warnings.push(CssCheckLintFinding {
                        code: "LINT001",
                        line: Some(lint_line),
                        column: Some(lint_column),
                        message: format!(
                            "duplicate @define-color '{}' (later definition overrides earlier)",
                            name
                        ),
                    });
                }
            }
        }
        offset += segment.len();
    }

    // Selector repeats matter across the whole file
    let mut selector_seen: HashMap<String, usize> = HashMap::new();
    lint_css_block(
        &stripped,
        &stripped,
        0,
        None,
        custom_properties,
        &mut selector_seen,
        &mut warnings,
    );
    warnings
}

fn lint_css_block(
    contents: &str,
    source_contents: &str,
    base_offset: usize,
    context: Option<String>,
    custom_properties: &CssCustomPropertyScopes,
    selector_seen: &mut HashMap<String, usize>,
    warnings: &mut Vec<CssCheckLintFinding>,
) {
    // Nested blocks reuse the full file text so line math stays absolute
    let mut cursor = 0usize;
    let bytes = contents.as_bytes();
    while let Some(css_block) = next_css_block_with_offsets(bytes, cursor) {
        cursor = css_block.next;
        let selector = normalize_selector(&css_block.selector);
        if selector.is_empty() {
            continue;
        }

        if selector.starts_with('@') {
            if should_recurse_at_rule(&selector) {
                let nested_context = match context.as_ref() {
                    Some(parent) => format!("{parent} {selector}"),
                    None => selector.clone(),
                };
                // Keep the at-rule in the warning so the scope still makes sense
                lint_css_block(
                    &css_block.block,
                    source_contents,
                    base_offset + css_block.block_start,
                    Some(nested_context),
                    custom_properties,
                    selector_seen,
                    warnings,
                );
            }
            continue;
        }

        // Grouped selectors still need one warning per real selector
        for (selector_part, selector_offset) in selector_part_locations(&css_block.selector) {
            if selector_part.is_empty() {
                continue;
            }
            let key = match context.as_ref() {
                Some(prefix) => format!("{prefix}::{selector_part}"),
                None => selector_part.clone(),
            };
            let count = selector_seen.entry(key).or_insert(0);
            *count += 1;
            if *count > 1 {
                let context_note = context
                    .as_ref()
                    .map(|ctx| format!(" within {ctx}"))
                    .unwrap_or_default();
                let (lint_line, lint_column) = line_column_for_offset(
                    source_contents,
                    base_offset + css_block.selector_start + selector_offset,
                );
                warnings.push(CssCheckLintFinding {
                    code: "LINT002",
                    line: Some(lint_line),
                    column: Some(lint_column),
                    message: format!(
                        "duplicate selector '{}'{} (later rules override earlier)",
                        selector_part, context_note
                    ),
                });
            }
        }

        warnings.extend(lint_css_properties(
            source_contents,
            &selector,
            &css_block.block,
            base_offset + css_block.block_start,
            context.as_deref(),
            custom_properties,
        ));
    }
}

fn lint_css_properties(
    contents: &str,
    selector: &str,
    block: &str,
    block_start: usize,
    context: Option<&str>,
    custom_properties: &CssCustomPropertyScopes,
) -> Vec<CssCheckLintFinding> {
    let mut warnings = Vec::new();
    let mut seen: HashMap<String, String> = HashMap::new();
    for declaration in parse_css_declarations_with_offsets(block) {
        // Property offsets are relative to the block, so the block start gets added back here
        let prop = declaration.name;
        let value = declaration.value;
        let (lint_line, lint_column) =
            line_column_for_offset(contents, block_start + declaration.start);
        if let Some(previous_value) = seen.get(&prop) {
            let context_note = context
                .map(|ctx| format!(" within {ctx}"))
                .unwrap_or_default();
            if !is_deliberate_modern_fallback(previous_value, &value) {
                warnings.push(CssCheckLintFinding {
                    code: "LINT003",
                    line: Some(lint_line),
                    column: Some(lint_column),
                    message: format!(
                        "duplicate property '{}' in selector '{}'{} (later value overrides earlier)",
                        prop, selector, context_note
                    ),
                });
            }
        }
        seen.insert(prop.to_string(), value.clone());

        if let Some(message) =
            web_length_value_warning(&prop, &value, selector, context, custom_properties)
        {
            warnings.push(CssCheckLintFinding {
                code: "LINT004",
                line: Some(lint_line),
                column: Some(lint_column),
                message,
            });
        }
    }
    warnings
}

fn is_deliberate_modern_fallback(previous: &str, current: &str) -> bool {
    let previous = previous.trim();
    let current = current.trim();

    if previous == current {
        return true;
    }

    let previous_is_modern = previous.contains("var(") || previous.contains("calc(");
    let current_is_modern = current.contains("var(") || current.contains("calc(");

    !previous_is_modern && current_is_modern
}

fn generated_theme_token_css() -> Option<String> {
    let config_path = Config::default_config_path().ok()?;
    let config = Config::load_from_path(&config_path).ok()?;
    Some(build_modern_theme_custom_properties(
        &config.theme,
        gtk_css_features_for_version(4, 16),
    ))
}

fn web_length_value_warning(
    property: &str,
    value: &str,
    selector: &str,
    context: Option<&str>,
    custom_properties: &CssCustomPropertyScopes,
) -> Option<String> {
    if !is_horizontal_size_property(property) {
        return None;
    }

    if !value.contains('%')
        && can_model_horizontal_size_value(selector, property, value, custom_properties)
    {
        // Values the shared geometry parser can resolve do not need a fallback lint warning
        return None;
    }

    let hint = if value.contains('%') {
        // Percentages still depend on parent geometry that css-check does not model
        Some("uses percentage lengths, so geometry estimates may be incomplete")
    } else if value.contains("calc(") {
        // This stays noisy only when the shared geometry parser could not follow the math
        Some("uses calc() in a layout value that css-check could not resolve reliably")
    } else if value.contains("var(") {
        // This stays noisy only when the shared geometry parser could not follow the token chain
        Some("uses var() in a layout value that css-check could not resolve reliably")
    } else {
        None
    }?;

    let context_note = context
        .map(|ctx| format!(" within {ctx}"))
        .unwrap_or_default();
    Some(format!(
        "property '{}' in selector '{}'{} {}",
        property, selector, context_note, hint
    ))
}

fn selector_part_locations(selector: &str) -> Vec<(String, usize)> {
    let mut parts = Vec::new();
    let mut search_start = 0usize;
    for raw_part in split_selectors(selector) {
        let normalized = normalize_selector(&raw_part);
        if normalized.is_empty() {
            continue;
        }

        let offset = selector[search_start..]
            .find(raw_part.as_str())
            .map(|index| search_start + index)
            .unwrap_or(search_start);
        // Search resumes after the current match so repeated selectors still get the right offset
        search_start = offset + raw_part.len();
        parts.push((normalized, offset));
    }
    parts
}

fn line_column_for_offset(contents: &str, offset: usize) -> (usize, usize) {
    // Byte offsets are enough here because css-check already works on UTF-8 source slices
    let mut line = 1usize;
    let mut column = 1usize;
    for (index, ch) in contents.char_indices() {
        if index >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}
