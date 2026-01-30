//! CSS validation and lint helpers for UnixNotis themes.

use anyhow::{anyhow, Context, Result};
use gtk::prelude::*;
use gtk::CssProvider;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use unixnotis_core::Config;

pub(crate) fn run_css_check() -> Result<()> {
    // GTK initialization must happen before CSS parsing APIs are used.
    gtk::init().context("initialize gtk")?;
    let config_dir = Config::default_config_dir().context("resolve config directory")?;
    let display_root = display_config_root(&config_dir);
    if !config_dir.exists() {
        return Err(anyhow!("config directory not found: {}", display_root));
    }
    if !config_dir.is_dir() {
        return Err(anyhow!("config path is not a directory: {}", display_root));
    }
    // Collect CSS files once to keep linting/reporting consistent across steps.
    let css_files = collect_css_files(&config_dir)?;
    if css_files.is_empty() {
        return Err(anyhow!(
            "no css files found under {} (backup directories are skipped)",
            display_root
        ));
    }

    // Parsing errors are aggregated so a single failure can be reported succinctly.
    let error_count = Arc::new(AtomicUsize::new(0));
    let provider = CssProvider::new();
    let error_count_clone = error_count.clone();
    let config_root = config_dir.clone();
    let display_root_clone = display_root.clone();
    provider.connect_parsing_error(move |_provider, section, error| {
        error_count_clone.fetch_add(1, Ordering::Relaxed);
        let location = section.start_location();
        let file = section
            .file()
            .and_then(|file| file.path())
            .map(|path| format_display_path(&config_root, &display_root_clone, &path))
            .unwrap_or_else(|| "<data>".to_string());
        eprintln!(
            "css error: {}:{}:{}: {}",
            file,
            location.lines() + 1,
            location.line_chars() + 1,
            error.message()
        );
    });

    for path in &css_files {
        // File system validation avoids GTK parse attempts on invalid inputs.
        if !path.exists() {
            error_count.fetch_add(1, Ordering::Relaxed);
            let display_path = format_display_path(&config_dir, &display_root, path);
            eprintln!("css error: {}: file not found", display_path);
            continue;
        }
        if !path.is_file() {
            error_count.fetch_add(1, Ordering::Relaxed);
            let display_path = format_display_path(&config_dir, &display_root, path);
            eprintln!("css error: {}: not a regular file", display_path);
            continue;
        }
        provider.load_from_path(path);
    }

    // Parsing errors are fatal because GTK rejects invalid stylesheets.
    let errors = error_count.load(Ordering::Relaxed);
    if errors > 0 {
        return Err(anyhow!(
            "css-check found {} error(s) under {}",
            errors,
            display_root
        ));
    }

    // Warnings are advisory only; parsing errors remain fatal because GTK will refuse invalid CSS.
    // This keeps css-check strict about syntax while still surfacing override risks.
    // Lint warnings highlight override risks but do not fail the command.
    let warnings = lint_css_files(&css_files, &config_dir, &display_root)?;
    if warnings > 0 {
        println!(
            "css-check warnings: {} issue(s) under {}",
            warnings, display_root
        );
    }

    println!(
        "css-check ok: {} file(s) checked under {}",
        css_files.len(),
        display_root
    );
    Ok(())
}

fn collect_css_files(root: &Path) -> Result<Vec<PathBuf>> {
    // Depth-first traversal keeps allocations minimal while visiting all theme files.
    let mut visited: HashSet<PathBuf> = HashSet::new();
    let canonical_root = fs::canonicalize(root)
        .with_context(|| format!("resolve config directory {}", root.display()))?;
    // Canonicalize the root to keep symlink checks deterministic.
    visited.insert(canonical_root.clone());
    let mut stack = vec![root.to_path_buf()];
    let mut results = Vec::new();
    while let Some(dir) = stack.pop() {
        let entries = fs::read_dir(&dir)
            .with_context(|| format!("read config directory {}", dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let file_type = entry.file_type()?;
            let is_dir = if file_type.is_dir() {
                true
            } else if file_type.is_symlink() {
                // Symlinked directories are permitted for shared theme layouts.
                path.is_dir()
            } else {
                false
            };
            if is_dir {
                if is_backup_dir(&path) {
                    continue;
                }
                if let Ok(canonical) = fs::canonicalize(&path) {
                    // Restrict traversal to the config root even when symlinks are present.
                    if !canonical.starts_with(&canonical_root) {
                        continue;
                    }
                    if !visited.insert(canonical) {
                        // Already-visited directory, skip to prevent cycles.
                        continue;
                    }
                }
                stack.push(path);
            } else if is_css_file(&path) {
                results.push(path);
            }
        }
    }
    results.sort();
    Ok(results)
}

fn lint_css_files(files: &[PathBuf], config_dir: &Path, display_root: &str) -> Result<usize> {
    let mut warnings = 0usize;
    for path in files {
        let display_path = format_display_path(config_dir, display_root, path);
        // File contents are needed because GTK only reports parse errors, not override hazards.
        let contents = fs::read_to_string(path)
            .with_context(|| format!("read css file {}", path.display()))?;
        // The linter is intentionally shallow and low-cost; it avoids a full CSS parser.
        let report = lint_css_contents(&contents);
        for warning in report {
            warnings += 1;
            eprintln!("css warning: {}: {}", display_path, warning);
        }
    }
    Ok(warnings)
}

fn lint_css_contents(contents: &str) -> Vec<String> {
    let mut warnings = Vec::new();
    // Strip block comments first so selectors and properties are easier to scan.
    let stripped = strip_css_comments(contents);

    // Duplicate @define-color entries are allowed but usually accidental overrides.
    let mut color_defs: HashMap<String, usize> = HashMap::new();
    for line in stripped.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("@define-color") {
            if let Some(name) = rest.split_whitespace().next() {
                let count = color_defs.entry(name.to_string()).or_insert(0);
                *count += 1;
                if *count > 1 {
                    warnings.push(format!(
                        "duplicate @define-color '{}' (later definition overrides earlier)",
                        name
                    ));
                }
            }
        }
    }

    // Track selectors to flag redefinitions that silently override earlier rules.
    let mut selector_seen: HashMap<String, usize> = HashMap::new();
    lint_css_block(&stripped, None, &mut selector_seen, &mut warnings);

    warnings
}

fn lint_css_block(
    contents: &str,
    context: Option<String>,
    selector_seen: &mut HashMap<String, usize>,
    warnings: &mut Vec<String>,
) {
    let mut cursor = 0usize;
    let bytes = contents.as_bytes();
    while let Some((selector, block, next)) = next_css_block(bytes, cursor) {
        cursor = next;
        // Collapse whitespace so duplicates match even if formatting differs.
        let selector = normalize_selector(&selector);
        if selector.is_empty() {
            continue;
        }
        if selector.starts_with('@') {
            // Recurse into at-rules that contain nested selector blocks.
            if should_recurse_at_rule(&selector) {
                let nested_context = match context.as_ref() {
                    Some(parent) => format!("{parent} {selector}"),
                    None => selector.clone(),
                };
                // Propagate at-rule context so warnings can pinpoint scope.
                lint_css_block(&block, Some(nested_context), selector_seen, warnings);
            }
            continue;
        }
        // Split grouped selectors (".a, .b") so each rule is tracked independently.
        for selector_part in split_selectors(&selector) {
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
                warnings.push(format!(
                    "duplicate selector '{}'{} (later rules override earlier)",
                    selector_part, context_note
                ));
            }
        }
        // Property duplicates are flagged separately from selector duplicates.
        warnings.extend(lint_css_properties(&selector, &block, context.as_deref()));
    }
}

fn lint_css_properties(selector: &str, block: &str, context: Option<&str>) -> Vec<String> {
    let mut warnings = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Property duplicates within a selector are almost always accidental overrides.
    for chunk in block.split(';') {
        let trimmed = chunk.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some((name, _)) = trimmed.split_once(':') else {
            continue;
        };
        let prop = name.trim();
        if prop.is_empty() {
            continue;
        }
        let inserted = seen.insert(prop.to_string());
        if !inserted {
            let context_note = context
                .map(|ctx| format!(" within {ctx}"))
                .unwrap_or_default();
            warnings.push(format!(
                "duplicate property '{}' in selector '{}'{} (later value overrides earlier)",
                prop, selector, context_note
            ));
        }
    }
    warnings
}

fn strip_css_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_comment = false;
    while let Some(ch) = chars.next() {
        if in_comment {
            // Consume until the closing delimiter to avoid misparsing selectors.
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                chars.next();
                in_comment = false;
            }
            continue;
        }
        if ch == '/' && matches!(chars.peek(), Some('*')) {
            // Enter comment mode and skip delimiter.
            chars.next();
            in_comment = true;
            continue;
        }
        output.push(ch);
    }
    output
}

fn next_css_block(bytes: &[u8], start: usize) -> Option<(String, String, usize)> {
    // A lightweight brace scanner is sufficient for identifying selector blocks.
    // Comments are already stripped, so only string literals need to be respected.
    let mut selector_start = start;
    while selector_start < bytes.len() && bytes[selector_start].is_ascii_whitespace() {
        selector_start += 1;
    }
    let mut index = selector_start;
    let mut in_string: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote) = in_string {
            // Inside strings, only the matching quote can terminate the literal.
            if byte == quote {
                in_string = None;
            }
            index += 1;
            continue;
        }
        if byte == b'"' || byte == b'\'' {
            // Track string literals so braces inside strings do not confuse parsing.
            in_string = Some(byte);
            index += 1;
            continue;
        }
        if byte == b'{' {
            let selector = String::from_utf8_lossy(&bytes[selector_start..index]).to_string();
            // Nested braces can appear in at-rules; track depth to find the matching close.
            let mut depth = 1usize;
            index += 1;
            let block_start = index;
            while index < bytes.len() {
                let byte = bytes[index];
                if let Some(quote) = in_string {
                    // Strings can appear inside blocks as well, so track them too.
                    if byte == quote {
                        in_string = None;
                    }
                    index += 1;
                    continue;
                }
                if byte == b'"' || byte == b'\'' {
                    in_string = Some(byte);
                    index += 1;
                    continue;
                }
                if byte == b'{' {
                    // Increase depth for nested blocks.
                    depth += 1;
                } else if byte == b'}' {
                    // Close the current block when depth returns to zero.
                    depth -= 1;
                    if depth == 0 {
                        let block = String::from_utf8_lossy(&bytes[block_start..index]).to_string();
                        return Some((selector, block, index + 1));
                    }
                }
                index += 1;
            }
            break;
        }
        index += 1;
    }
    None
}

fn normalize_selector(selector: &str) -> String {
    // Normalize whitespace so the same selector compares equal across formatting styles.
    selector
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn split_selectors(selector: &str) -> Vec<String> {
    // Split on commas so grouped selectors are checked individually.
    // Respect parentheses/attribute selectors so ":is(.a, .b)" does not split.
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_string: Option<char> = None;
    let mut chars = selector.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(quote) = in_string {
            // Preserve escaped characters inside string literals.
            if ch == '\\' {
                current.push(ch);
                if let Some(next_char) = chars.next() {
                    current.push(next_char);
                }
                continue;
            }
            if ch == quote {
                in_string = None;
            }
            current.push(ch);
            continue;
        }

        match ch {
            '"' | '\'' => {
                // Strings can appear inside attribute selectors.
                in_string = Some(ch);
                current.push(ch);
            }
            '(' => {
                // Track parentheses depth to avoid splitting within :is() etc.
                paren_depth = paren_depth.saturating_add(1);
                current.push(ch);
            }
            ')' => {
                // Decrease depth for nested pseudo selectors.
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            '[' => {
                // Track attribute selector nesting.
                bracket_depth = bracket_depth.saturating_add(1);
                current.push(ch);
            }
            ']' => {
                // Close attribute selector nesting.
                bracket_depth = bracket_depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if paren_depth == 0 && bracket_depth == 0 => {
                // Only split on commas when outside nested constructs.
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }

    let trimmed = current.trim();
    if !trimmed.is_empty() {
        parts.push(trimmed.to_string());
    }

    parts
}

fn should_recurse_at_rule(selector: &str) -> bool {
    // Only recurse into at-rules that contain standard selector blocks.
    let name = selector
        .trim_start_matches('@')
        .split_whitespace()
        .next()
        .unwrap_or("");
    matches!(
        name,
        "media" | "supports" | "layer" | "container" | "document"
    )
}

fn is_backup_dir(path: &Path) -> bool {
    // Backup directories follow the Backup-YYYY-MM-DD pattern (with optional suffix).
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with("Backup-"))
        .unwrap_or(false)
}

fn is_css_file(path: &Path) -> bool {
    // CSS validation only applies to *.css files within the config tree.
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("css"))
        .unwrap_or(false)
}

fn display_config_root(config_dir: &Path) -> String {
    // Prefer stable env-rooted display paths for user-facing output.
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
        let trimmed = xdg.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.is_absolute() && config_dir == path.join("unixnotis") {
                // Keep output stable across machines by using env placeholders.
                return "$XDG_CONFIG_HOME/unixnotis".to_string();
            }
        }
    }
    if let Ok(home) = env::var("HOME") {
        let path = PathBuf::from(home).join(".config").join("unixnotis");
        if config_dir == path {
            return "$HOME/.config/unixnotis".to_string();
        }
    }
    config_dir.display().to_string()
}

fn format_display_path(config_dir: &Path, display_root: &str, path: &Path) -> String {
    // Shorten absolute paths to the config root when possible for cleaner output.
    if let Ok(relative) = path.strip_prefix(config_dir) {
        if relative.as_os_str().is_empty() {
            return display_root.to_string();
        }
        // Preserve the display root so logs stay consistent with XDG paths.
        return format!("{}/{}", display_root, relative.display());
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_selectors_handles_is_commas() {
        // Commas inside :is() should not split the selector list.
        let selector = ".a:is(.b, .c), .d";
        let parts = split_selectors(selector);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], ".a:is(.b, .c)");
        assert_eq!(parts[1], ".d");
    }

    #[test]
    fn lint_css_contents_scans_media_blocks() {
        // Duplicate selectors inside @media blocks should be detected with context.
        let css = "@media (min-width: 1px) { .a { color: red; } .a { color: blue; } }";
        let warnings = lint_css_contents(css);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("duplicate selector '.a'"));
        assert!(warnings[0].contains("within @media (min-width: 1px)"));
    }

    #[test]
    fn lint_css_contents_scans_layer_blocks() {
        // Duplicate selectors inside @layer blocks should be detected with context.
        let css = "@layer theme { .a { color: red; } .a { color: blue; } }";
        let warnings = lint_css_contents(css);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("duplicate selector '.a'"));
        assert!(warnings[0].contains("within @layer theme"));
    }
}
