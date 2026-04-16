use std::collections::HashMap;

use super::super::main_css_check_parse::{
    next_css_block_with_offsets, normalize_selector, parse_css_declarations_with_offsets,
    should_recurse_at_rule, split_selectors, strip_css_comments,
};
use super::values::{
    line_column_for_offset, should_suppress_duplicate_property_warning, web_length_value_warning,
};
use super::{CssCheckLintFinding, CssCustomPropertyScopes};

pub(super) fn lint_css_contents_with_properties(
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
                // At-rules still matter because duplicate selectors and bad layout values can
                // hide inside the nested block
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
            if !should_suppress_duplicate_property_warning(
                &prop,
                previous_value,
                &value,
                selector,
                custom_properties,
            ) {
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
        // The later declaration is what GTK keeps, so the duplicate check needs the same rule
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
