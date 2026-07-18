//! Stylesheet import validation and URL payload rewriting

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use unixnotis_core::{
    collect_css_import_dependency_values, collect_css_import_url_spans, collect_css_url_spans,
    CssImportReference,
};

use super::materialize::CssAssetMaterializer;
use super::model::{ImportedCssReference, IncludedBundleFiles};
use super::reference::classify_imported_css_reference;
use crate::preset::archive::MAX_PRESET_FILE_BYTES;
use crate::preset::css_asset_refs::has_css_extension;

pub(super) fn rewrite_stylesheet(
    config_dir: &Path,
    available: &IncludedBundleFiles<'_>,
    materializer: &mut CssAssetMaterializer<'_>,
    css_relative_path: &Path,
    css_text: &str,
) -> Result<Vec<u8>> {
    validate_import_targets(config_dir, available, css_relative_path, css_text)?;
    let import_url_ranges = collect_css_import_url_spans(css_text)?
        .into_iter()
        .map(|span| (span.value_start, span.value_end))
        .collect::<BTreeSet<_>>();
    let spans = collect_css_url_spans(css_text)?;
    let mut rewritten = String::with_capacity(css_text.len());
    let mut last_index = 0usize;

    for span in spans {
        if span.ambiguous {
            return Err(anyhow!(
                "{} contains an ambiguous escaped CSS URL payload",
                css_relative_path.display()
            ));
        }
        rewritten.push_str(&css_text[last_index..span.value_start]);
        if import_url_ranges.contains(&(span.value_start, span.value_end)) {
            // Imported stylesheets stay CSS and are validated separately
            rewritten.push_str(&span.value);
        } else {
            let replacement =
                materializer.materialize_reference(available, css_relative_path, &span.value)?;
            rewritten.push_str(replacement.as_deref().unwrap_or(&span.value));
        }
        last_index = span.value_end;
    }
    rewritten.push_str(&css_text[last_index..]);

    validate_rewritten_css_size(rewritten.len() as u64, css_relative_path)?;
    Ok(rewritten.into_bytes())
}

pub(super) fn validate_rewritten_css_size(size: u64, css_relative_path: &Path) -> Result<()> {
    if size > MAX_PRESET_FILE_BYTES {
        return Err(anyhow!(
            "rewritten CSS file exceeds {MAX_PRESET_FILE_BYTES} bytes: {}",
            css_relative_path.display()
        ));
    }
    Ok(())
}

fn validate_import_targets(
    config_dir: &Path,
    available: &IncludedBundleFiles<'_>,
    css_relative_path: &Path,
    css_text: &str,
) -> Result<()> {
    for reference in collect_css_import_dependency_values(css_text)? {
        let CssImportReference::Target(value) = reference else {
            return Err(anyhow!(
                "{} contains an ambiguous CSS import target",
                css_relative_path.display()
            ));
        };
        match classify_imported_css_reference(
            config_dir,
            css_relative_path,
            &value,
            MAX_PRESET_FILE_BYTES,
        )? {
            ImportedCssReference::Bundled(relative) => {
                if !has_css_extension(&relative) {
                    return Err(anyhow!(
                        "CSS import target must use a .css extension: {}",
                        relative.display()
                    ));
                }
                let file = available.get(&relative).ok_or_else(|| {
                    anyhow!(
                        "CSS import target is not included in the preset: {}",
                        relative.display()
                    )
                })?;
                std::str::from_utf8(&file.contents).with_context(|| {
                    format!(
                        "CSS import target is not valid UTF-8: {}",
                        relative.display()
                    )
                })?;
            }
            ImportedCssReference::Data { .. } => {
                return Err(anyhow!("CSS @import does not accept data image URLs"));
            }
            ImportedCssReference::External => {
                // The caller applies the explicit external-reference policy afterward
            }
        }
    }
    Ok(())
}
