//! CSS url(...) asset rebasing helpers

use std::path::{Component, Path, PathBuf};

use gtk::gio;
use gtk::prelude::FileExt;

pub(super) fn rebase_relative_css_asset_urls(contents: &str, css_path: &Path) -> String {
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
    let tail = input.get(open_index..)?;

    // Leading spaces after url( are ignored so stored payloads stay clean
    let (first_value_offset, first_value) = tail
        .char_indices()
        .find(|(_, ch)| !ch.is_ascii_whitespace())?;
    let mut index = open_index + first_value_offset;

    let mut value = String::new();
    let mut value_end;
    let mut quote = None::<char>;
    let mut closed_quote = false;
    if matches!(first_value, '\'' | '"') {
        // Quoted URLs keep the quote out of the stored payload and later rewrite
        quote = Some(first_value);
        index += first_value.len_utf8();
    }
    let value_start = index;
    value_end = index;

    for (offset, ch) in input[index..].char_indices() {
        let index = index + offset;
        if let Some(open_quote) = quote {
            if ch == open_quote {
                quote = None;
                // Padding after a valid quoted URL is CSS syntax, not part of the asset path
                closed_quote = true;
            } else {
                value.push(ch);
                value_end = index + ch.len_utf8();
            }
            continue;
        }

        match ch {
            ')' => {
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
            ch if closed_quote && ch.is_ascii_whitespace() => {
                // Ignore normal whitespace between the closing quote and `)`
            }
            _ => {
                // Once malformed suffix text begins, later spaces are user bytes again
                closed_quote = false;
                value.push(ch);
                if !ch.is_ascii_whitespace() {
                    value_end = index + ch.len_utf8();
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
#[path = "../tests/loader/paths.rs"]
mod path_tests;
#[cfg(test)]
#[path = "../tests/loader/rebase.rs"]
mod rebase_tests;
#[cfg(test)]
#[path = "../tests/loader/spans.rs"]
mod span_tests;
#[cfg(test)]
#[path = "../tests/loader/url_parser.rs"]
mod url_parser_tests;
