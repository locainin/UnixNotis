use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::{self, File, OpenOptions};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use tar::{Builder, Header};

use super::super::config_root::CollectedConfigFiles;
use super::super::manifest::PresetManifest;
use super::super::pathing::{archive_payload_path, MANIFEST_ARCHIVE_PATH};
use super::modes::sanitize_payload_mode;
use super::read::{
    MAX_PRESET_FILE_BYTES, MAX_PRESET_MANIFEST_BYTES, MAX_PRESET_PAYLOAD_FILES,
    MAX_PRESET_TOTAL_PAYLOAD_BYTES,
};

pub fn write_bundle(
    bundle_path: &Path,
    manifest: &PresetManifest,
    collected: &CollectedConfigFiles,
) -> Result<()> {
    validate_export_payload_sizes(collected)?;
    if let Some(parent) = bundle_path.parent() {
        // Export can target nested output paths, so create the parent tree first
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create preset parent directory {}", parent.display()))?;
    }

    // Writing into a temp file first keeps partial bundles and symlink-follow writes off the target path
    let temp_path = temp_bundle_path(bundle_path);
    let output = create_temp_bundle_file(&temp_path)?;
    let encoder = GzEncoder::new(output, Compression::default());
    let mut builder = Builder::new(encoder);

    // Manifest always goes in first so a partial or broken bundle is easy to spot
    let manifest_bytes = manifest.encode()?.into_bytes();
    validate_export_manifest_size(manifest_bytes.len() as u64)?;
    append_bytes(
        &mut builder,
        Path::new(MANIFEST_ARCHIVE_PATH),
        &manifest_bytes,
        0o644,
    )?;

    for file in &collected.files {
        let mode = sanitize_payload_mode(file.mode, &file.relative_path)?;

        // Every payload comes from bytes captured through the secure config-root descriptor
        let contents = file
            .contents_override
            .as_deref()
            .unwrap_or(&file.source_contents);
        append_bytes(
            &mut builder,
            &archive_payload_path(&file.relative_path),
            contents,
            mode,
        )?;
    }

    // Finish the tar writer first, then flush the gzip stream to disk
    builder.finish().context("finish preset archive")?;
    let encoder = builder
        .into_inner()
        .context("flush preset archive writer")?;
    let output = encoder.finish().context("finalize preset bundle")?;
    output
        .sync_all()
        .with_context(|| format!("flush temp preset bundle {}", temp_path.display()))?;

    if let Err(err) = fs::rename(&temp_path, bundle_path)
        .with_context(|| format!("replace preset bundle {}", bundle_path.display()))
    {
        // Temp bundle cleanup keeps failed exports from leaving large junk files behind
        let _ = fs::remove_file(&temp_path);
        return Err(err);
    }
    Ok(())
}

pub(super) fn validate_export_manifest_size(size: u64) -> Result<()> {
    if size > MAX_PRESET_MANIFEST_BYTES {
        anyhow::bail!("preset export manifest exceeds {MAX_PRESET_MANIFEST_BYTES} bytes");
    }
    Ok(())
}

pub(super) fn validate_export_payload_sizes(collected: &CollectedConfigFiles) -> Result<()> {
    if collected.files.len() > MAX_PRESET_PAYLOAD_FILES {
        anyhow::bail!("preset export contains more than {MAX_PRESET_PAYLOAD_FILES} files");
    }
    let mut total = 0u64;
    for file in &collected.files {
        let size = file
            .contents_override
            .as_ref()
            .map_or(file.source_contents.len(), Vec::len) as u64;
        if size > MAX_PRESET_FILE_BYTES {
            anyhow::bail!(
                "preset export file exceeds {MAX_PRESET_FILE_BYTES} bytes: {}",
                file.relative_path.display()
            );
        }
        total = total
            .checked_add(size)
            .filter(|value| *value <= MAX_PRESET_TOTAL_PAYLOAD_BYTES)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "preset export payload exceeds {MAX_PRESET_TOTAL_PAYLOAD_BYTES} bytes"
                )
            })?;
    }
    Ok(())
}

fn append_bytes(
    builder: &mut Builder<GzEncoder<File>>,
    path: &Path,
    contents: &[u8],
    mode: u32,
) -> Result<()> {
    // Small in-memory writes are enough for the manifest
    let mut header = Header::new_gnu();
    header.set_mode(mode);
    header.set_size(contents.len() as u64);
    header.set_cksum();
    builder
        .append_data(&mut header, path, Cursor::new(contents))
        .with_context(|| format!("append {} to preset archive", path.display()))?;
    Ok(())
}

pub(super) fn temp_bundle_path(bundle_path: &Path) -> PathBuf {
    let parent = bundle_path
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let file_name = bundle_path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("preset.unixnotis");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock moved backwards")
        .as_nanos();
    parent.join(format!(".{file_name}.{stamp}.tmp"))
}

pub(super) fn create_temp_bundle_file(temp_path: &Path) -> Result<File> {
    // create_new refuses existing files and symlinks, so temp writes cannot be redirected
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temp_path)
        .with_context(|| format!("create temp preset bundle {}", temp_path.display()))
}
