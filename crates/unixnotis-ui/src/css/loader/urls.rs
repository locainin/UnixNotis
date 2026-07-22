//! CSS url(...) asset rebasing helpers

use std::path::{Component, Path, PathBuf};

use gtk::gio;
use gtk::prelude::FileExt;
use unixnotis_core::{collect_css_url_spans, has_valid_percent_encoding};
use url::Url;

pub(super) fn rebase_relative_css_asset_urls(contents: &str, css_path: &Path) -> String {
    let mut rewritten = String::with_capacity(contents.len());
    let mut last_index = 0usize;

    // Each url(...) payload is inspected in-place so the rest of the stylesheet stays untouched
    let Ok(spans) = collect_css_url_spans(contents) else {
        // Invalid CSS remains byte-for-byte intact for the provider's normal diagnostics
        return contents.to_string();
    };
    for span in spans {
        rewritten.push_str(&contents[last_index..span.value_start]);
        if span.ambiguous {
            // Ambiguous payload escapes stay untouched instead of becoming a guessed file path
            rewritten.push_str(&span.value);
        } else if let Some(asset_uri) = rebase_relative_asset_ref_to_file_uri(&span.value, css_path)
        {
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

    if Url::parse(trimmed).is_ok() {
        // Absolute URIs already carry their own scheme and must not become local filesystem paths
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
    if has_valid_percent_encoding(trimmed.as_bytes()) {
        let normalized_base = normalize_lexical_path(base_dir);
        let base_url = Url::from_directory_path(normalized_base).ok()?;
        let resolved_url = base_url.join(trimmed).ok()?;
        if resolved_url.scheme() == "file"
            && resolved_url.host_str().is_none()
            && resolved_url.query().is_none()
            && resolved_url.fragment().is_none()
        {
            // URL joining decodes the portable reference only when GTK opens the final file URI
            return Some(resolved_url.into());
        }
    }
    // GTK understands file:// URIs even when the provider is loaded from raw merged CSS bytes
    Some(gio::File::for_path(resolved).uri().to_string())
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
                Some(Component::RootDir | Component::Prefix(_)) => {}
                // Leading `..` must be preserved when there is nothing earlier to fold away
                _ => normalized.push(".."),
            },
        }
    }
    normalized
}

#[cfg(test)]
#[path = "tests/paths.rs"]
mod path_tests;
#[cfg(test)]
#[path = "tests/rebase.rs"]
mod rebase_tests;
