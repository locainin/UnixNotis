//! Allocation-safe scan of tar headers hidden by extension processing

use std::fs::File;
use std::io::{Read, Seek};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use flate2::read::GzDecoder;
use tar::{Header, PaxExtensions};

use super::budget::DecompressedBudget;
use super::read::{validate_entry_size, MAX_PRESET_FILE_BYTES, MAX_PRESET_MANIFEST_BYTES};
use crate::preset::pathing::{archive_payload_relative, MANIFEST_ARCHIVE_PATH};

pub(super) const MAX_PRESET_ARCHIVE_ENTRIES: usize = 2_048;
pub(super) const MAX_PRESET_EXTENSION_METADATA_BYTES: u64 = 1_048_576;

pub(super) fn preflight_archive(input: &mut File, decompressed_limit: u64) -> Result<()> {
    {
        let decoder = GzDecoder::new(&mut *input);
        let mut bounded = DecompressedBudget::new(decoder, decompressed_limit);
        scan_headers(&mut bounded)?;
    }

    // Reuse the validated descriptor so a path swap cannot select another bundle
    input
        .rewind()
        .context("rewind preset bundle after raw archive preflight")?;
    Ok(())
}

fn scan_headers(input: &mut impl Read) -> Result<()> {
    let mut entry_count = 0usize;
    let mut pax_size = None::<u64>;

    while let Some(header) = read_header(input)? {
        entry_count += 1;
        if entry_count > MAX_PRESET_ARCHIVE_ENTRIES {
            return Err(anyhow!(
                "preset bundle contains too many archive entries during raw preflight: max {MAX_PRESET_ARCHIVE_ENTRIES}"
            ));
        }

        let entry_type = header.entry_type();
        let recognized = header.as_gnu().is_some() || header.as_ustar().is_some();
        let is_hidden_extension = recognized
            && (entry_type.is_gnu_longname()
                || entry_type.is_gnu_longlink()
                || entry_type.is_pax_local_extensions());
        let raw_size = header.entry_size().context("read raw tar entry size")?;

        if is_hidden_extension {
            if raw_size > MAX_PRESET_EXTENSION_METADATA_BYTES {
                return Err(anyhow!(
                    "preset bundle extension metadata is too large: {raw_size} bytes, max {MAX_PRESET_EXTENSION_METADATA_BYTES} bytes"
                ));
            }

            if entry_type.is_pax_local_extensions() {
                if pax_size.is_some() {
                    return Err(anyhow!(
                        "preset bundle contains consecutive local PAX headers"
                    ));
                }
                let contents = read_entry_contents(input, raw_size)?;
                pax_size = parse_pax_size(&contents)?;
            } else {
                skip_entry_contents(input, raw_size)?;
            }
            continue;
        }

        // PAX size controls the following ordinary member's physical body length
        let effective_size = pax_size.take().unwrap_or(raw_size);
        validate_visible_header(&header, effective_size)?;
        skip_entry_contents(input, effective_size)?;
    }

    Ok(())
}

fn read_header(input: &mut impl Read) -> Result<Option<Header>> {
    let mut header = Header::new_old();
    let bytes = header.as_mut_bytes();
    let mut filled = 0usize;

    while filled < bytes.len() {
        let read = input
            .read(&mut bytes[filled..])
            .context("read raw tar header")?;
        if read == 0 {
            if filled == 0 {
                return Ok(None);
            }
            return Err(anyhow!("preset bundle ended inside a tar header"));
        }
        filled += read;
    }

    if bytes.iter().all(|byte| *byte == 0) {
        return Ok(None);
    }
    validate_header_checksum(&header)?;
    Ok(Some(header))
}

fn validate_header_checksum(header: &Header) -> Result<()> {
    let expected = header.cksum().context("read raw tar header checksum")?;
    let bytes = header.as_bytes();
    // Tar calculates the checksum as though its eight checksum bytes were spaces
    let actual = bytes[..148]
        .iter()
        .chain(&bytes[156..])
        .fold(8 * u32::from(b' '), |sum, byte| sum + u32::from(*byte));
    if actual != expected {
        return Err(anyhow!("preset bundle contains a tar checksum mismatch"));
    }
    Ok(())
}

fn parse_pax_size(contents: &[u8]) -> Result<Option<u64>> {
    let mut size = None;
    for extension in PaxExtensions::new(contents) {
        let extension = extension.context("parse local PAX metadata")?;
        if extension.key_bytes() != b"size" || size.is_some() {
            continue;
        }
        let value =
            std::str::from_utf8(extension.value_bytes()).context("PAX size is not valid UTF-8")?;
        size = Some(value.parse::<u64>().context("PAX size is not a number")?);
    }
    Ok(size)
}

fn validate_visible_header(header: &Header, effective_size: u64) -> Result<()> {
    let archive_path = header
        .path()
        .context("read raw preset bundle entry path")?
        .into_owned();
    if header.entry_type().is_dir() {
        if effective_size != 0 {
            return Err(anyhow!(
                "preset bundle directory entry has a nonzero size: {}",
                archive_path.display()
            ));
        }
        return Ok(());
    }
    if archive_path == Path::new(MANIFEST_ARCHIVE_PATH) {
        return validate_entry_size(
            "manifest",
            &archive_path,
            effective_size,
            MAX_PRESET_MANIFEST_BYTES,
        );
    }
    if archive_payload_relative(&archive_path)?.is_some() {
        validate_entry_size(
            "payload",
            &archive_path,
            effective_size,
            MAX_PRESET_FILE_BYTES,
        )?;
    }
    Ok(())
}

fn read_entry_contents(input: &mut impl Read, size: u64) -> Result<Vec<u8>> {
    let length = usize::try_from(size).context("extension metadata size does not fit memory")?;
    let mut contents = vec![0_u8; length];
    input
        .read_exact(&mut contents)
        .context("read tar extension metadata")?;
    skip_exact(input, padding_bytes(size))?;
    Ok(contents)
}

fn skip_entry_contents(input: &mut impl Read, size: u64) -> Result<()> {
    skip_exact(input, size)?;
    skip_exact(input, padding_bytes(size))
}

const fn padding_bytes(size: u64) -> u64 {
    let remainder = size % 512;
    if remainder == 0 {
        return 0;
    }
    512 - remainder
}

fn skip_exact(input: &mut impl Read, mut remaining: u64) -> Result<()> {
    // A small fixed buffer keeps skip memory flat for large members
    let mut buffer = [0_u8; 8_192];
    while remaining != 0 {
        let length = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(buffer.len());
        input
            .read_exact(&mut buffer[..length])
            .context("skip tar entry contents")?;
        let consumed = u64::try_from(length).context("skip buffer length does not fit u64")?;
        remaining -= consumed;
    }
    Ok(())
}
