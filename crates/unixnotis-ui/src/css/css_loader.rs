//! CSS file loading helpers with fallback and override handling.

use std::fs;
use std::path::{Component, Path, PathBuf};

use gtk::gio;
use gtk::prelude::FileExt;
use tracing::warn;

/// Load CSS into a provider, applying overrides and falling back to defaults.
pub(crate) fn load_provider_with_overrides(
    load_css_data: impl Fn(&str),
    path: &Path,
    fallback: &str,
    overrides: &str,
    inject_base_tokens: bool,
) {
    match fs::read_to_string(path) {
        Ok(contents) => {
            let contents = if inject_base_tokens {
                ensure_base_tokens(&contents, path)
            } else {
                contents
            };
            if contents.trim().is_empty() {
                // Empty files fall back to embedded defaults so windows stay styled.
                let merged = merge_css_with_overrides(fallback, fallback, overrides);
                // Relative url(...) assets break when CSS is loaded from raw bytes,
                // so rebase them against the stylesheet path before GTK sees the data
                load_css_data(&rebase_relative_css_asset_urls(&merged, path));
                return;
            }
            let merged = merge_css_with_overrides(&contents, fallback, overrides);
            // The provider still loads merged data, but the asset URLs now point at real files
            load_css_data(&rebase_relative_css_asset_urls(&merged, path));
        }
        Err(err) => {
            let file = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("css");
            warn!(
                ?err,
                file, "failed to read css file; falling back to defaults"
            );
            let fallback = if inject_base_tokens {
                ensure_base_tokens(fallback, path)
            } else {
                fallback.to_string()
            };
            if overrides.trim().is_empty() {
                // Fallback CSS can carry relative assets too, so it needs the same rebasing path
                load_css_data(&rebase_relative_css_asset_urls(&fallback, path));
                return;
            }
            let merged = format!("{fallback}\n{overrides}");
            // Overrides are merged before rebasing so later asset refs all see one final stylesheet
            load_css_data(&rebase_relative_css_asset_urls(&merged, path));
        }
    }
}

fn merge_css_with_overrides(contents: &str, fallback: &str, overrides: &str) -> String {
    if overrides.trim().is_empty() {
        return contents.to_string();
    }

    // User overrides are appended to untouched defaults and prepended to user-edited files
    if contents.trim() == fallback.trim() {
        format!("{contents}\n{overrides}")
    } else {
        format!("{overrides}\n{contents}")
    }
}

pub(crate) fn ensure_base_tokens(contents: &str, path: &Path) -> String {
    if contents.contains("unixnotis-surface-base") && contents.contains("unixnotis-card-base") {
        return contents.to_string();
    }
    let file = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("base.css");
    warn!(
        file,
        "base css missing base color tokens; alpha overrides may be compounded until updated"
    );
    format!(
        "{prefix}\n{contents}",
        prefix = r#"@define-color unixnotis-surface-base @unixnotis-surface;
@define-color unixnotis-surface-strong-base @unixnotis-surface-strong;
@define-color unixnotis-card-base @unixnotis-card;"#,
    )
}

fn rebase_relative_css_asset_urls(contents: &str, css_path: &Path) -> String {
    let mut rewritten = String::with_capacity(contents.len());
    let mut last_index = 0usize;

    // Each url(...) payload is inspected in-place so the rest of the stylesheet stays untouched
    for span in collect_url_spans(contents) {
        rewritten.push_str(&contents[last_index..span.value_start]);
        if let Some(asset_uri) = rebase_relative_asset_ref_to_file_uri(&span.value, css_path) {
            rewritten.push_str(&asset_uri);
        } else {
            rewritten.push_str(&span.value);
        }
        last_index = span.value_end;
    }

    rewritten.push_str(&contents[last_index..]);
    rewritten
}

fn rebase_relative_asset_ref_to_file_uri(asset_ref: &str, css_path: &Path) -> Option<String> {
    let trimmed = asset_ref.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("data:")
        || lowered.starts_with("http://")
        || lowered.starts_with("https://")
        || lowered.starts_with("file://")
    {
        // Embedded data, remote URLs, and absolute file URLs already carry their own target
        return None;
    }

    let relative = Path::new(trimmed);
    if relative.is_absolute() {
        // Absolute filesystem paths are already explicit and do not need CSS rebasing here
        return None;
    }

    // Relative CSS asset refs are anchored to the stylesheet directory, not the process cwd
    let base_dir = css_path.parent()?;
    let resolved = normalize_lexical_path(&base_dir.join(relative));
    // GTK understands file:// URIs even when the provider is loaded from raw merged CSS bytes
    Some(gio::File::for_path(resolved).uri().to_string())
}

fn collect_url_spans(css_text: &str) -> Vec<UrlValueSpan> {
    let bytes = css_text.as_bytes();
    let mut spans = Vec::new();
    let mut in_comment = false;
    let mut skip_until = 0usize;

    // Byte-based scanning keeps rewrite ranges exact while `for` guarantees forward progress
    for index in 0..bytes.len() {
        if index < skip_until {
            continue;
        }
        if in_comment {
            // Comment bodies should never produce fake url(...) matches
            if bytes.get(index..index.saturating_add(2)) == Some(b"*/") {
                in_comment = false;
            }
            continue;
        }

        if bytes.get(index..index.saturating_add(2)) == Some(b"/*") {
            in_comment = true;
            continue;
        }

        if starts_with_url(bytes, index) {
            let open_index = index + 4;
            let Some((span, next_index)) = parse_url_value(css_text, open_index) else {
                break;
            };
            spans.push(span);
            skip_until = next_index;
        }
    }

    spans
}

fn starts_with_url(bytes: &[u8], index: usize) -> bool {
    // ASCII-only matching avoids slicing through UTF-8 code points
    index + 4 <= bytes.len()
        && bytes[index].eq_ignore_ascii_case(&b'u')
        && bytes[index + 1].eq_ignore_ascii_case(&b'r')
        && bytes[index + 2].eq_ignore_ascii_case(&b'l')
        && bytes[index + 3] == b'('
}

struct UrlValueSpan {
    // Raw url(...) payload after outer quotes and spacing are stripped away
    value: String,
    // Byte range inside the original CSS string where the payload lived
    value_start: usize,
    value_end: usize,
}

fn parse_url_value(input: &str, open_index: usize) -> Option<(UrlValueSpan, usize)> {
    let bytes = input.as_bytes();
    let tail = bytes.get(open_index..)?;

    // Leading spaces after url( are ignored so stored payloads stay clean
    let first_value_offset = tail.iter().position(|byte| !byte.is_ascii_whitespace())?;
    let mut index = open_index + first_value_offset;

    let mut value = String::new();
    let mut value_end;
    let mut quote = None::<u8>;
    let mut closed_quote = false;
    if matches!(bytes[index], b'\'' | b'"') {
        // Quoted URLs keep the quote out of the stored payload and later rewrite
        quote = Some(bytes[index]);
        index += 1;
    }
    let value_start = index;
    value_end = index;

    for (index, byte) in bytes.iter().copied().enumerate().skip(index) {
        if let Some(open_quote) = quote {
            if byte == open_quote {
                quote = None;
                // Padding after a valid quoted URL is CSS syntax, not part of the asset path
                closed_quote = true;
            } else {
                value.push(byte as char);
                value_end = index + 1;
            }
            continue;
        }

        match byte {
            b')' => {
                // Closing paren ends the payload and returns the exact slice that was replaced
                return Some((
                    UrlValueSpan {
                        value: value.trim().to_string(),
                        value_start,
                        value_end,
                    },
                    index + 1,
                ));
            }
            byte if closed_quote && byte.is_ascii_whitespace() => {
                // Ignore normal whitespace between the closing quote and `)`
            }
            _ => {
                value.push(byte as char);
                if !byte.is_ascii_whitespace() {
                    value_end = index + 1;
                }
            }
        }
    }

    None
}

fn normalize_lexical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => match normalized.components().next_back() {
                Some(Component::Normal(_)) => {
                    // One parent segment cancels one earlier normal segment when that is possible
                    normalized.pop();
                }
                Some(Component::RootDir) | Some(Component::Prefix(_)) => {}
                // Leading `..` must be preserved when there is nothing earlier to fold away
                _ => normalized.push(".."),
            },
        }
    }
    normalized
}

#[cfg(test)]
#[path = "tests/loader_merge.rs"]
mod merge_tests;
#[cfg(test)]
#[path = "tests/loader_paths.rs"]
mod path_tests;
#[cfg(test)]
#[path = "tests/loader_provider.rs"]
mod provider_tests;
#[cfg(test)]
#[path = "tests/loader_rebase.rs"]
mod rebase_tests;
#[cfg(test)]
#[path = "tests/loader_spans.rs"]
mod span_tests;
#[cfg(test)]
#[path = "tests/loader_tokens.rs"]
mod token_tests;
#[cfg(test)]
#[path = "tests/loader_url_parser.rs"]
mod url_parser_tests;
