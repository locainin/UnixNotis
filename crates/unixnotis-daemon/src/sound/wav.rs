//! Structural validation for notification-supplied PCM WAVE files

use std::fs;
use std::os::unix::fs::FileExt;

const RIFF_HEADER_BYTES: u64 = 12;
const CHUNK_HEADER_BYTES: u64 = 8;
const PCM_FORMAT_BYTES: u32 = 16;
const MAX_WAV_CHUNKS: usize = 1_024;
const MIN_SAMPLE_RATE: u32 = 8_000;
const MAX_SAMPLE_RATE: u32 = 192_000;
const MAX_CHANNELS: u16 = 2;

#[derive(Clone, Copy)]
struct PcmFormat {
    block_align: u16,
}

pub(super) fn is_safe_pcm_wav(file: &fs::File, file_len: u64) -> bool {
    let mut riff_header = [0u8; RIFF_HEADER_BYTES as usize];
    if file.read_exact_at(&mut riff_header, 0).is_err()
        || &riff_header[..4] != b"RIFF"
        || &riff_header[8..] != b"WAVE"
    {
        return false;
    }

    // RIFF size excludes the leading identifier and size field
    let Some(declared_len) = read_u32(&riff_header[4..8]).map(|size| u64::from(size) + 8) else {
        return false;
    };
    if declared_len != file_len {
        return false;
    }

    let mut cursor = RIFF_HEADER_BYTES;
    let mut pcm_format = None;
    let mut found_data = false;
    let mut chunk_count = 0usize;

    while cursor < file_len {
        chunk_count += 1;
        if chunk_count > MAX_WAV_CHUNKS {
            return false;
        }

        let mut chunk_header = [0u8; CHUNK_HEADER_BYTES as usize];
        if file.read_exact_at(&mut chunk_header, cursor).is_err() {
            return false;
        }
        let Some(chunk_size) = read_u32(&chunk_header[4..]) else {
            return false;
        };
        let data_start = match cursor.checked_add(CHUNK_HEADER_BYTES) {
            Some(offset) => offset,
            None => return false,
        };
        let data_end = match data_start.checked_add(u64::from(chunk_size)) {
            Some(offset) if offset <= file_len => offset,
            _ => return false,
        };
        // RIFF chunks use one padding byte after odd-sized payloads
        let padded_end = match data_end.checked_add(u64::from(chunk_size & 1)) {
            Some(offset) if offset <= file_len => offset,
            _ => return false,
        };

        match &chunk_header[..4] {
            b"fmt " => {
                // Multiple format declarations create decoder-dependent interpretation
                if pcm_format.is_some() {
                    return false;
                }
                pcm_format = read_pcm_format(file, data_start, chunk_size);
                if pcm_format.is_none() {
                    return false;
                }
            }
            b"data" => {
                // The format must be known before audio bytes are accepted
                let Some(format) = pcm_format else {
                    return false;
                };
                if found_data || chunk_size == 0 || chunk_size % u32::from(format.block_align) != 0
                {
                    return false;
                }
                found_data = true;
            }
            _ => {}
        }

        cursor = padded_end;
    }

    cursor == file_len && pcm_format.is_some() && found_data
}

fn read_pcm_format(file: &fs::File, offset: u64, chunk_size: u32) -> Option<PcmFormat> {
    // Restrict file hints to the fixed-size canonical PCM format block
    if chunk_size != PCM_FORMAT_BYTES {
        return None;
    }
    let mut format = [0u8; PCM_FORMAT_BYTES as usize];
    file.read_exact_at(&mut format, offset).ok()?;

    let audio_format = read_u16(&format[0..2])?;
    let channels = read_u16(&format[2..4])?;
    let sample_rate = read_u32(&format[4..8])?;
    let byte_rate = read_u32(&format[8..12])?;
    let block_align = read_u16(&format[12..14])?;
    let bits_per_sample = read_u16(&format[14..16])?;

    if audio_format != 1
        || !(1..=MAX_CHANNELS).contains(&channels)
        || !(MIN_SAMPLE_RATE..=MAX_SAMPLE_RATE).contains(&sample_rate)
        || !matches!(bits_per_sample, 8 | 16 | 24 | 32)
    {
        return None;
    }

    let bytes_per_sample = bits_per_sample.checked_div(8)?;
    let expected_align = channels.checked_mul(bytes_per_sample)?;
    let expected_rate = sample_rate.checked_mul(u32::from(expected_align))?;
    if block_align != expected_align || byte_rate != expected_rate {
        return None;
    }

    Some(PcmFormat { block_align })
}

fn read_u16(bytes: &[u8]) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.try_into().ok()?))
}

fn read_u32(bytes: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

#[cfg(test)]
#[path = "tests/wav.rs"]
mod tests;
