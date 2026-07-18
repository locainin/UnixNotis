//! Bundle-wide stylesheet transformation orchestration

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use super::bundle::{append_generated_assets, validate_transformed_bundle};
use super::materialize::CssAssetMaterializer;
use super::model::IncludedBundleFiles;
use super::rewrite::rewrite_stylesheet;
use crate::preset::archive::BundleFile;
use crate::preset::css_asset_refs::has_css_extension;
use crate::preset::pathing::relative_path_matches_exclusion;

pub(in crate::preset::import) fn harden_imported_css_assets(
    config_dir: &Path,
    files: &mut Vec<BundleFile>,
    exclusions: &[PathBuf],
) -> Result<()> {
    let available = files
        .iter()
        .filter(|file| !relative_path_matches_exclusion(&file.relative_path, exclusions))
        .map(|file| (file.relative_path.clone(), file))
        .collect::<IncludedBundleFiles<'_>>();
    let stylesheets = available
        .iter()
        .filter(|(path, _file)| has_css_extension(path))
        .map(|(path, _file)| path.clone())
        .collect::<Vec<_>>();

    let mut materializer = CssAssetMaterializer::new(config_dir);
    let mut rewritten = BTreeMap::new();
    for path in stylesheets {
        let file = available
            .get(&path)
            .ok_or_else(|| anyhow!("stylesheet disappeared from the available bundle map"))?;
        let text = std::str::from_utf8(&file.contents)
            .with_context(|| format!("stylesheet is not valid UTF-8: {}", path.display()))?;
        rewritten.insert(
            path.clone(),
            rewrite_stylesheet(config_dir, &available, &mut materializer, &path, text)?,
        );
    }
    let generated = materializer.into_generated();

    for file in files.iter_mut() {
        if let Some(contents) = rewritten.remove(&file.relative_path) {
            file.contents = contents;
        }
    }
    if !rewritten.is_empty() {
        return Err(anyhow!(
            "could not locate a rewritten stylesheet in the bundle"
        ));
    }

    append_generated_assets(files, exclusions, generated)?;
    validate_transformed_bundle(files)
}
