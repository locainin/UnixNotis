//! CSS asset reference checks shared by preset flows and css-check
//!
//! These helpers look for `url(...)` references that reach outside the
//! UnixNotis config root so import and export can warn before a shared preset
//! depends on host-local files

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use unixnotis_core::util;

use super::archive::BundleFile;
use super::config_root::PresetFileSource;
use super::pathing::normalize_lexical_path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExternalCssAssetRef {
    // CSS file that carried the outside asset reference
    pub(crate) css_file: PathBuf,
    // Raw url(...) payload as written in the stylesheet
    pub(crate) asset_ref: String,
    // Short reason shown back to the caller
    pub(crate) reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostSpecificCssAssetRef {
    // CSS file that carried the host-local config path
    pub(crate) css_file: PathBuf,
    // Raw url(...) payload as written in the stylesheet
    pub(crate) asset_ref: String,
    // Replacement path written into the bundled stylesheet
    pub(crate) rewritten_ref: String,
}

pub(super) fn collect_external_css_asset_refs_from_bundle(
    config_dir: &Path,
    files: &[BundleFile],
) -> Vec<ExternalCssAssetRef> {
    let mut refs = Vec::new();

    // Bundle files are already in memory, so this scan is cheap and keeps import warnings early
    for file in files {
        if !has_css_extension(&file.relative_path) {
            continue;
        }
        let css_path = config_dir.join(&file.relative_path);
        let css_text = String::from_utf8_lossy(&file.contents);
        refs.extend(collect_external_refs_from_text(
            config_dir,
            &css_path,
            css_text.as_ref(),
        ));
    }

    refs
}

pub(super) fn collect_external_css_asset_refs_from_collected(
    config_dir: &Path,
    files: &[PresetFileSource],
) -> Result<Vec<ExternalCssAssetRef>> {
    let mut refs = Vec::new();

    // Export may already have in-memory overrides, so scan the effective bundle bytes here
    for file in files {
        if !has_css_extension(&file.relative_path) {
            continue;
        }

        let css_text = read_css_text(file)?;
        refs.extend(collect_external_refs_from_text(
            config_dir,
            &file.source_path,
            &css_text,
        ));
    }

    Ok(refs)
}

pub(super) fn rewrite_host_specific_css_asset_refs_in_sources(
    config_dir: &Path,
    files: &mut [PresetFileSource],
) -> Result<Vec<HostSpecificCssAssetRef>> {
    let mut rewrites = Vec::new();

    for file in files {
        if !has_css_extension(&file.relative_path) {
            continue;
        }

        let css_text = read_css_text(file)?;
        let (rewritten_text, file_rewrites) =
            rewrite_host_specific_refs_in_text(config_dir, &file.source_path, &css_text);
        if file_rewrites.is_empty() {
            continue;
        }

        // Export keeps the rewritten stylesheet in memory so the live file stays untouched
        file.size = rewritten_text.len() as u64;
        file.contents_override = Some(rewritten_text.into_bytes());
        rewrites.extend(file_rewrites);
    }

    Ok(rewrites)
}

pub(crate) fn collect_external_css_asset_refs_from_paths(
    config_dir: &Path,
    css_paths: &[PathBuf],
) -> Result<Vec<ExternalCssAssetRef>> {
    let mut refs = Vec::new();

    for css_path in css_paths {
        let css_text = std::fs::read_to_string(css_path)
            .with_context(|| format!("read css file {}", css_path.display()))?;
        refs.extend(collect_external_refs_from_text(
            config_dir, css_path, &css_text,
        ));
    }

    Ok(refs)
}

fn collect_external_refs_from_text(
    config_dir: &Path,
    css_path: &Path,
    css_text: &str,
) -> Vec<ExternalCssAssetRef> {
    let mut refs = Vec::new();
    let stripped = strip_css_comments(css_text);

    for asset_ref in collect_url_values(&stripped) {
        if let Some(reason) = classify_external_asset_ref(config_dir, css_path, &asset_ref) {
            refs.push(ExternalCssAssetRef {
                css_file: css_path.to_path_buf(),
                asset_ref,
                reason,
            });
        }
    }

    refs
}

fn rewrite_host_specific_refs_in_text(
    config_dir: &Path,
    css_path: &Path,
    css_text: &str,
) -> (String, Vec<HostSpecificCssAssetRef>) {
    let mut rewritten = String::with_capacity(css_text.len());
    let mut rewrites = Vec::new();
    let mut last_index = 0usize;

    for span in collect_url_spans(css_text) {
        rewritten.push_str(&css_text[last_index..span.value_start]);

        if let Some(rewritten_ref) =
            rewrite_host_specific_asset_ref(config_dir, css_path, &span.value)
        {
            rewritten.push_str(&rewritten_ref);
            rewrites.push(HostSpecificCssAssetRef {
                css_file: css_path.to_path_buf(),
                asset_ref: span.value,
                rewritten_ref,
            });
        } else {
            rewritten.push_str(&span.value);
        }

        last_index = span.value_end;
    }

    rewritten.push_str(&css_text[last_index..]);
    (rewritten, rewrites)
}

fn classify_external_asset_ref(
    config_dir: &Path,
    css_path: &Path,
    asset_ref: &str,
) -> Option<String> {
    let trimmed = asset_ref.trim();
    if trimmed.is_empty() {
        return None;
    }

    let lowered = trimmed.to_ascii_lowercase();
    if lowered.starts_with("data:") {
        // Embedded data stays self-contained inside the CSS file
        return None;
    }
    if lowered.starts_with("http://") || lowered.starts_with("https://") {
        return Some("remote url".to_string());
    }
    if let Some(path) = local_file_url_path(trimmed) {
        return asset_path_reason(config_dir, &path);
    }

    let expanded = PathBuf::from(util::expand_tilde(trimmed).into_owned());
    if expanded.is_absolute() {
        return asset_path_reason(config_dir, &expanded);
    }

    let base_dir = css_path.parent().unwrap_or(config_dir);
    let resolved = normalize_lexical_path(&base_dir.join(expanded));
    let normalized_root = normalize_lexical_path(config_dir);
    if !resolved.starts_with(&normalized_root) {
        return Some("relative path leaves the config root".to_string());
    }

    None
}

fn rewrite_host_specific_asset_ref(
    config_dir: &Path,
    css_path: &Path,
    asset_ref: &str,
) -> Option<String> {
    let trimmed = asset_ref.trim();
    if trimmed.is_empty() {
        return None;
    }

    let asset_path = if let Some(path) = local_file_url_path(trimmed) {
        path
    } else {
        let expanded = PathBuf::from(util::expand_tilde(trimmed).into_owned());
        if !expanded.is_absolute() {
            return None;
        }
        expanded
    };

    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_asset = normalize_lexical_path(&asset_path);
    let relative_asset = normalized_asset.strip_prefix(&normalized_root).ok()?;

    // CSS paths should stay stylesheet-relative so they work after import on any machine
    let css_base_dir = css_path.parent().unwrap_or(config_dir);
    let normalized_css_base = normalize_lexical_path(css_base_dir);
    Some(relative_css_path(
        &normalized_css_base,
        &normalized_root.join(relative_asset),
    ))
}

fn asset_path_reason(config_dir: &Path, candidate: &Path) -> Option<String> {
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_candidate = normalize_lexical_path(candidate);
    if normalized_candidate.starts_with(&normalized_root) {
        return None;
    }
    Some("local path points outside the config root".to_string())
}

fn local_file_url_path(value: &str) -> Option<PathBuf> {
    let path = value.strip_prefix("file://")?;
    // file://localhost/path is a local file URL too
    let path = path.strip_prefix("localhost/").unwrap_or(path);
    if !path.starts_with('/') {
        return None;
    }
    Some(PathBuf::from(path))
}

fn collect_url_values(css_text: &str) -> Vec<String> {
    collect_url_spans(css_text)
        .into_iter()
        .map(|span| span.value)
        .collect()
}

struct UrlValueSpan {
    value: String,
    value_start: usize,
    value_end: usize,
}

fn collect_url_spans(css_text: &str) -> Vec<UrlValueSpan> {
    let bytes = css_text.as_bytes();
    let mut spans = Vec::new();
    let mut index = 0usize;
    let mut in_comment = false;

    while index < bytes.len() {
        if in_comment {
            if index + 1 < bytes.len() && bytes[index] == b'*' && bytes[index + 1] == b'/' {
                in_comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }

        if index + 1 < bytes.len() && bytes[index] == b'/' && bytes[index + 1] == b'*' {
            in_comment = true;
            index += 2;
            continue;
        }

        if starts_with_url(bytes, index) {
            let open_index = index + 4;
            let Some((span, next_index)) = parse_url_value(css_text, open_index) else {
                break;
            };
            spans.push(span);
            index = next_index;
            continue;
        }

        index += 1;
    }

    spans
}

fn starts_with_url(bytes: &[u8], index: usize) -> bool {
    // URL matching stays ASCII-only so the scanner never slices through UTF-8 code points
    index + 4 <= bytes.len()
        && bytes[index].eq_ignore_ascii_case(&b'u')
        && bytes[index + 1].eq_ignore_ascii_case(&b'r')
        && bytes[index + 2].eq_ignore_ascii_case(&b'l')
        && bytes[index + 3] == b'('
}

fn parse_url_value(input: &str, open_index: usize) -> Option<(UrlValueSpan, usize)> {
    let bytes = input.as_bytes();
    let mut index = open_index;

    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    if index >= bytes.len() {
        return None;
    }

    let mut value = String::new();
    let mut value_end;
    let mut quote = None::<u8>;
    if matches!(bytes[index], b'\'' | b'"') {
        quote = Some(bytes[index]);
        index += 1;
    }
    let value_start = index;
    value_end = index;

    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(open_quote) = quote {
            if byte == open_quote {
                quote = None;
            } else {
                value.push(byte as char);
                value_end = index + 1;
            }
            index += 1;
            continue;
        }

        match byte {
            b')' => {
                return Some((
                    UrlValueSpan {
                        value: value.trim().to_string(),
                        value_start,
                        value_end,
                    },
                    index + 1,
                ));
            }
            b'\'' | b'"' => {
                // Unquoted url(...) should stop at the closing paren
                value.push(byte as char);
                value_end = index + 1;
            }
            _ => {
                value.push(byte as char);
                if !byte.is_ascii_whitespace() || !value.trim().is_empty() {
                    value_end = index + 1;
                }
            }
        }
        index += 1;
    }

    None
}

fn strip_css_comments(input: &str) -> Cow<'_, str> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_comment = false;
    let mut changed = false;

    while let Some(ch) = chars.next() {
        if in_comment {
            if ch == '*' && matches!(chars.peek(), Some('/')) {
                chars.next();
                in_comment = false;
            }
            changed = true;
            continue;
        }
        if ch == '/' && matches!(chars.peek(), Some('*')) {
            chars.next();
            in_comment = true;
            changed = true;
            continue;
        }
        output.push(ch);
    }

    if changed {
        Cow::Owned(output)
    } else {
        Cow::Borrowed(input)
    }
}

fn has_css_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("css"))
        .unwrap_or(false)
}

fn read_css_text(file: &PresetFileSource) -> Result<String> {
    if let Some(contents) = &file.contents_override {
        return String::from_utf8(contents.clone())
            .with_context(|| format!("decode css override {}", file.relative_path.display()));
    }

    std::fs::read_to_string(&file.source_path)
        .with_context(|| format!("read css file {}", file.source_path.display()))
}

fn relative_css_path(base_dir: &Path, target_path: &Path) -> String {
    let base_parts = base_dir
        .components()
        .filter_map(normal_component)
        .collect::<Vec<_>>();
    let target_parts = target_path
        .components()
        .filter_map(normal_component)
        .collect::<Vec<_>>();

    let mut shared = 0usize;
    while shared < base_parts.len()
        && shared < target_parts.len()
        && base_parts[shared] == target_parts[shared]
    {
        shared += 1;
    }

    let mut relative = PathBuf::new();
    for _ in shared..base_parts.len() {
        relative.push("..");
    }
    for part in &target_parts[shared..] {
        relative.push(part);
    }

    format_css_relative_path(&relative)
}

fn normal_component(component: std::path::Component<'_>) -> Option<String> {
    match component {
        std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
        _ => None,
    }
}

fn format_css_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::ParentDir => Some("..".to_string()),
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::{
        collect_external_css_asset_refs_from_bundle, collect_external_css_asset_refs_from_paths,
    };
    use crate::preset::archive::BundleFile;
    use std::fs;
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
            let path = std::env::temp_dir().join(format!(
                "unixnotis-css-asset-refs-{}-{}-{}",
                name, stamp, serial
            ));
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative_path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create parent dirs");
            }
            fs::write(&path, contents).expect("write file");
            path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn finds_file_url_outside_root_in_bundle_css() {
        let root = TempDirGuard::new("bundle");
        let config_dir = root.path.join("xdg/unixnotis");
        fs::create_dir_all(&config_dir).expect("create config dir");

        let refs = collect_external_css_asset_refs_from_bundle(
            &config_dir,
            &[BundleFile {
                relative_path: PathBuf::from("base.css"),
                contents: b".panel { background-image: url(\"file:///tmp/outside.png\"); }\n"
                    .to_vec(),
                mode: 0o644,
            }],
        );

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].reason, "local path points outside the config root");
    }

    #[test]
    fn finds_relative_parent_escape_in_live_css() {
        let root = TempDirGuard::new("relative");
        let config_dir = root.path.join("xdg/unixnotis");
        fs::create_dir_all(&config_dir).expect("create config dir");
        let css_path = root.write(
            "xdg/unixnotis/base.css",
            ".panel { background-image: url(\"../outside.png\"); }\n",
        );

        let refs =
            collect_external_css_asset_refs_from_paths(&config_dir, &[css_path]).expect("scan css");

        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].reason, "relative path leaves the config root");
    }
}
