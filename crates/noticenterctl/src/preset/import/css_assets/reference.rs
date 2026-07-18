//! Imported CSS reference classification and bounded data URL decoding

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use data_url::DataUrl;
use unixnotis_core::has_valid_percent_encoding;
use url::Url;

use super::model::ImportedCssReference;
use crate::preset::css_asset_refs::{classify_file_url, FileUrlClassification};
use crate::preset::pathing::normalize_lexical_path;

pub(super) fn classify_imported_css_reference(
    config_dir: &Path,
    css_relative_path: &Path,
    value: &str,
    max_data_bytes: u64,
) -> Result<ImportedCssReference> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        return Err(anyhow!(
            "CSS asset reference is empty or contains a null byte"
        ));
    }

    match classify_file_url(trimmed) {
        FileUrlClassification::NotFileUrl => {}
        // File URLs remain outside the portable preset boundary even when locally hosted
        FileUrlClassification::Local(_)
        | FileUrlClassification::NonLocalAuthority
        | FileUrlClassification::Malformed => return Ok(ImportedCssReference::External),
    }

    if trimmed
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        return decode_data_image(trimmed, max_data_bytes);
    }
    if Url::parse(trimmed).is_ok() || Path::new(trimmed).is_absolute() || trimmed.starts_with('~') {
        return Ok(ImportedCssReference::External);
    }

    let css_path = config_dir.join(css_relative_path);
    let base_dir = css_path.parent().unwrap_or(config_dir);
    let resolved = resolve_relative_url_path(base_dir, trimmed)?;
    let normalized_root = normalize_lexical_path(config_dir);
    let normalized_target = normalize_lexical_path(&resolved);
    let Ok(relative) = normalized_target.strip_prefix(&normalized_root) else {
        return Ok(ImportedCssReference::External);
    };
    if relative.as_os_str().is_empty() {
        return Err(anyhow!(
            "relative CSS asset path resolves to the config directory"
        ));
    }
    Ok(ImportedCssReference::Bundled(relative.to_path_buf()))
}

fn resolve_relative_url_path(base_dir: &Path, value: &str) -> Result<PathBuf> {
    if has_valid_percent_encoding(value.as_bytes()) {
        let normalized_base = normalize_lexical_path(base_dir);
        let base_url = Url::from_directory_path(normalized_base)
            .map_err(|()| anyhow!("could not serialize the CSS base directory as a file URL"))?;
        let resolved_url = base_url
            .join(value)
            .with_context(|| format!("resolve relative CSS URL {value}"))?;
        if resolved_url.query().is_some() || resolved_url.fragment().is_some() {
            return Err(anyhow!(
                "relative CSS asset URLs with queries or fragments are not portable"
            ));
        }
        return resolved_url
            .to_file_path()
            .map_err(|()| anyhow!("relative CSS URL does not resolve to a local file"));
    }

    // Invalid percent escapes retain literal filesystem meaning for compatibility
    Ok(normalize_lexical_path(&base_dir.join(value)))
}

fn decode_data_image(value: &str, max_bytes: u64) -> Result<ImportedCssReference> {
    let data = DataUrl::process(value).context("parse CSS data URL")?;
    let extension = image_extension_for_mime(data.mime_type())?;
    let max_bytes =
        usize::try_from(max_bytes).context("CSS image byte limit does not fit memory")?;
    let mut contents = Vec::new();
    let fragment = data
        .decode(|chunk| -> Result<()> {
            let next_len = contents
                .len()
                .checked_add(chunk.len())
                .ok_or_else(|| anyhow!("CSS data image size overflow"))?;
            if next_len > max_bytes {
                return Err(anyhow!(
                    "CSS data image exceeds the {max_bytes} byte decoded-content limit"
                ));
            }
            contents
                .try_reserve(chunk.len())
                .context("reserve bounded CSS data image bytes")?;
            contents.extend_from_slice(chunk);
            Ok(())
        })
        .map_err(|error| anyhow!("decode CSS data image: {error}"))?;
    if fragment.is_some() {
        return Err(anyhow!("CSS data image fragments are not supported"));
    }

    Ok(ImportedCssReference::Data {
        path_hint: PathBuf::from(format!("inline.{extension}")),
        contents,
    })
}

fn image_extension_for_mime(mime: &data_url::mime::Mime) -> Result<&'static str> {
    if mime.matches("image", "png") {
        return Ok("png");
    }
    if mime.matches("image", "jpeg") {
        return Ok("jpg");
    }
    if mime.matches("image", "webp") {
        return Ok("webp");
    }
    if mime.matches("image", "svg+xml") {
        return Ok("svg");
    }
    Err(anyhow!(
        "CSS data URL uses unsupported media type {}/{}",
        mime.type_,
        mime.subtype
    ))
}
