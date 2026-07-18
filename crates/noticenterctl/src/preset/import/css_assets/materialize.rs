//! Bounded image decoding, PNG generation, and portable URL serialization

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use unixnotis_core::{
    materialize_bounded_image_as_png, AssetPolicy, DEFAULT_ICON_ASSET_EXTENSIONS,
};
use url::Url;

use super::model::{CssAssetSourceKey, ImportedCssReference, IncludedBundleFiles};
use super::reference::classify_imported_css_reference;
use crate::preset::archive::{
    MAX_PRESET_FILE_BYTES, MAX_PRESET_PAYLOAD_FILES, MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};

const CSS_IMAGE_MAX_WIDTH: u32 = 4_096;
const CSS_IMAGE_MAX_HEIGHT: u32 = 4_096;
const CSS_IMAGE_MAX_PIXELS: u64 = 8_388_608;
const MATERIALIZED_ASSET_DIR: &str = "assets/.validated-css";

pub(super) struct CssAssetMaterializer<'a> {
    config_dir: &'a Path,
    generated: BTreeMap<PathBuf, Vec<u8>>,
    generated_bytes: u64,
    source_cache: BTreeMap<CssAssetSourceKey, PathBuf>,
    limits: MaterializationLimits,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct MaterializationLimits {
    pub(super) max_files: usize,
    pub(super) max_bytes: u64,
}

impl<'a> CssAssetMaterializer<'a> {
    pub(super) const fn new(config_dir: &'a Path) -> Self {
        Self::with_limits(
            config_dir,
            MaterializationLimits {
                max_files: MAX_PRESET_PAYLOAD_FILES,
                max_bytes: MAX_PRESET_TOTAL_PAYLOAD_BYTES,
            },
        )
    }

    pub(super) const fn with_limits(config_dir: &'a Path, limits: MaterializationLimits) -> Self {
        Self {
            config_dir,
            generated: BTreeMap::new(),
            generated_bytes: 0,
            source_cache: BTreeMap::new(),
            limits,
        }
    }

    pub(super) fn materialize_reference(
        &mut self,
        available: &IncludedBundleFiles<'_>,
        css_relative_path: &Path,
        value: &str,
    ) -> Result<Option<String>> {
        let target = classify_imported_css_reference(
            self.config_dir,
            css_relative_path,
            value,
            MAX_PRESET_FILE_BYTES,
        )?;
        let (source_key, path_hint) = match &target {
            ImportedCssReference::Bundled(relative) => {
                let file = available.get(relative).ok_or_else(|| {
                    anyhow!(
                        "CSS image target is not included in the preset: {}",
                        relative.display()
                    )
                })?;
                if file.mode & 0o111 != 0 {
                    return Err(anyhow!(
                        "CSS image target must not be executable: {}",
                        relative.display()
                    ));
                }
                (
                    CssAssetSourceKey::Bundled(relative.clone()),
                    relative.clone(),
                )
            }
            ImportedCssReference::Data {
                path_hint,
                contents,
            } => {
                let digest = *blake3::hash(contents).as_bytes();
                (CssAssetSourceKey::Data(digest), path_hint.clone())
            }
            ImportedCssReference::External => return Ok(None),
        };

        let generated_path = if let Some(path) = self.source_cache.get(&source_key) {
            path.clone()
        } else {
            let source_contents = match &target {
                ImportedCssReference::Bundled(relative) => {
                    &available
                        .get(relative)
                        .ok_or_else(|| {
                            anyhow!(
                                "CSS image target disappeared from the preset: {}",
                                relative.display()
                            )
                        })?
                        .contents
                }
                ImportedCssReference::Data { contents, .. } => contents,
                ImportedCssReference::External => return Ok(None),
            };
            let png =
                materialize_bounded_image_as_png(&path_hint, source_contents, css_image_policy())
                    .with_context(|| format!("validate CSS image {}", path_hint.display()))?;
            validate_materialized_png_size(png.len() as u64)?;
            let digest = blake3::hash(&png).to_hex();
            let path = Path::new(MATERIALIZED_ASSET_DIR).join(format!("{digest}.png"));
            self.store_generated_png(path.clone(), png)?;
            self.source_cache.insert(source_key, path.clone());
            path
        };

        relative_url_from_stylesheet(css_relative_path, &generated_path).map(Some)
    }

    pub(super) fn into_generated(self) -> BTreeMap<PathBuf, Vec<u8>> {
        self.generated
    }

    fn store_generated_png(&mut self, path: PathBuf, png: Vec<u8>) -> Result<()> {
        if self.generated.contains_key(&path) {
            return Ok(());
        }
        if self.generated.len() >= self.limits.max_files {
            return Err(anyhow!(
                "validated CSS images exceed the {} generated-file limit",
                self.limits.max_files
            ));
        }
        let next_bytes = self
            .generated_bytes
            .checked_add(png.len() as u64)
            .ok_or_else(|| anyhow!("validated CSS image byte count overflow"))?;
        if next_bytes > self.limits.max_bytes {
            return Err(anyhow!(
                "validated CSS images exceed the {} generated-byte limit",
                self.limits.max_bytes
            ));
        }
        self.generated.insert(path, png);
        self.generated_bytes = next_bytes;
        Ok(())
    }
}

pub(super) fn validate_materialized_png_size(size: u64) -> Result<()> {
    if size > MAX_PRESET_FILE_BYTES {
        return Err(anyhow!(
            "validated CSS image exceeds {MAX_PRESET_FILE_BYTES} bytes after PNG encoding"
        ));
    }
    Ok(())
}

const fn css_image_policy() -> AssetPolicy {
    AssetPolicy {
        max_bytes: MAX_PRESET_FILE_BYTES,
        max_width: CSS_IMAGE_MAX_WIDTH,
        max_height: CSS_IMAGE_MAX_HEIGHT,
        max_pixels: CSS_IMAGE_MAX_PIXELS,
        allowed_extensions: DEFAULT_ICON_ASSET_EXTENSIONS,
    }
}

fn relative_url_from_stylesheet(stylesheet: &Path, target: &Path) -> Result<String> {
    let base_relative = stylesheet.parent().unwrap_or_else(|| Path::new(""));
    let base_url = Url::from_directory_path(Path::new("/").join(base_relative))
        .map_err(|()| anyhow!("could not serialize the stylesheet directory as a URL"))?;
    let target_url = Url::from_file_path(Path::new("/").join(target))
        .map_err(|()| anyhow!("could not serialize the validated CSS image path as a URL"))?;
    base_url
        .make_relative(&target_url)
        .ok_or_else(|| anyhow!("could not make the validated CSS image path relative"))
}
