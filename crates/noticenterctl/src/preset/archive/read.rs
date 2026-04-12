use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tar::Archive;

use super::super::manifest::PRESET_FORMAT_VERSION;
use super::super::pathing::{
    archive_payload_relative, format_relative_path, MANIFEST_ARCHIVE_PATH,
};
use super::modes::sanitize_payload_mode;
use super::{BundleArchive, BundleFile};

pub(crate) fn read_bundle(bundle_path: &Path) -> Result<BundleArchive> {
    // Import and inspect use the same reader so validation stays consistent
    let input = File::open(bundle_path)
        .with_context(|| format!("open preset bundle {}", bundle_path.display()))?;
    let decoder = GzDecoder::new(input);
    let mut archive = Archive::new(decoder);

    let mut manifest_contents = None::<String>;
    let mut files = BTreeMap::<PathBuf, BundleFile>::new();

    for entry in archive.entries().context("read preset bundle entries")? {
        let mut entry = entry.context("read preset bundle entry")?;
        if entry.header().entry_type().is_dir() {
            // Tar archives can carry directory records, but preset logic only cares about files
            continue;
        }

        let archive_path = entry.path().context("read bundle entry path")?.into_owned();

        if archive_path == Path::new(MANIFEST_ARCHIVE_PATH) {
            // Manifest text is read as UTF-8 so later checks can reason about field values
            let mut contents = String::new();
            entry
                .read_to_string(&mut contents)
                .context("read preset manifest")?;
            manifest_contents = Some(contents);
            continue;
        }

        let Some(relative_path) = archive_payload_relative(&archive_path)? else {
            // Only payload entries are turned into bundle files
            continue;
        };
        if !entry.header().entry_type().is_file() {
            return Err(anyhow!(
                "preset bundle contains a non-file payload entry: {}",
                archive_path.display()
            ));
        }
        if files.contains_key(&relative_path) {
            // Duplicate payload paths would make import order-sensitive, so reject them
            return Err(anyhow!(
                "preset bundle contains duplicate payload entry: {}",
                format_relative_path(&relative_path)
            ));
        }

        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .with_context(|| format!("read bundle payload {}", archive_path.display()))?;
        let mode = sanitize_payload_mode(entry.header().mode().unwrap_or(0o644), &relative_path)?;

        files.insert(
            relative_path.clone(),
            BundleFile {
                relative_path,
                contents,
                mode,
            },
        );
    }

    let manifest_contents =
        manifest_contents.ok_or_else(|| anyhow!("preset bundle is missing manifest.toml"))?;
    // Decode first, then fail hard on unsupported format versions
    let manifest = super::super::manifest::PresetManifest::decode(&manifest_contents)
        .context("decode preset manifest")?;
    if manifest.format_version != PRESET_FORMAT_VERSION {
        return Err(anyhow!(
            "unsupported preset format version {}",
            manifest.format_version
        ));
    }

    let expected_paths = manifest
        .files
        .iter()
        .map(|file| (PathBuf::from(&file.path), file.size))
        .collect::<BTreeMap<_, _>>();
    let actual_paths = files
        .iter()
        .map(|(path, file)| (path.clone(), file.contents.len() as u64))
        .collect::<BTreeMap<_, _>>();
    if expected_paths != actual_paths {
        // Import trusts the manifest file list, so a mismatch means the bundle is corrupt
        let expected = expected_paths
            .keys()
            .map(|path| format_relative_path(path))
            .collect::<BTreeSet<_>>();
        let actual = actual_paths
            .keys()
            .map(|path| format_relative_path(path))
            .collect::<BTreeSet<_>>();
        return Err(anyhow!(
            "preset manifest file list does not match archive payload\nexpected: {:?}\nactual: {:?}",
            expected,
            actual
        ));
    }

    Ok(BundleArchive {
        manifest,
        files: files.into_values().collect(),
    })
}
