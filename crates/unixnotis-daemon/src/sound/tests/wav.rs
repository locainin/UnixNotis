use super::*;
use crate::test_support::TempRoot;
use std::io::Write;

fn chunk(name: &[u8; 4], contents: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + contents.len() + (contents.len() & 1));
    bytes.extend_from_slice(name);
    bytes.extend_from_slice(
        &u32::try_from(contents.len())
            .expect("chunk size")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(contents);
    if contents.len() & 1 == 1 {
        bytes.push(0);
    }
    bytes
}

fn pcm_format(channels: u16, sample_rate: u32, bits_per_sample: u16) -> Vec<u8> {
    let block_align = channels * (bits_per_sample / 8);
    let byte_rate = sample_rate * u32::from(block_align);
    let mut format = Vec::with_capacity(16);
    format.extend_from_slice(&1u16.to_le_bytes());
    format.extend_from_slice(&channels.to_le_bytes());
    format.extend_from_slice(&sample_rate.to_le_bytes());
    format.extend_from_slice(&byte_rate.to_le_bytes());
    format.extend_from_slice(&block_align.to_le_bytes());
    format.extend_from_slice(&bits_per_sample.to_le_bytes());
    format
}

fn wave(chunks: &[Vec<u8>]) -> Vec<u8> {
    let payload_len = 4usize + chunks.iter().map(Vec::len).sum::<usize>();
    let mut bytes = Vec::with_capacity(payload_len + 8);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(
        &u32::try_from(payload_len)
            .expect("RIFF payload size")
            .to_le_bytes(),
    );
    bytes.extend_from_slice(b"WAVE");
    for item in chunks {
        bytes.extend_from_slice(item);
    }
    bytes
}

fn validate(bytes: &[u8]) -> bool {
    let root = TempRoot::new("sound-wav-parser");
    let path = root.join("sound.wav");
    let mut file = fs::File::create(path).expect("create WAVE fixture");
    file.write_all(bytes).expect("write WAVE fixture");
    drop(file);
    let file = fs::File::open(root.join("sound.wav")).expect("open WAVE fixture");
    is_safe_pcm_wav(&file, bytes.len() as u64)
}

fn canonical_wave() -> Vec<u8> {
    wave(&[
        chunk(b"fmt ", &pcm_format(1, 44_100, 16)),
        chunk(b"data", &[0; 2]),
    ])
}

#[test]
fn riff_and_wave_identifiers_are_validated_independently() {
    let mut wrong_riff = canonical_wave();
    wrong_riff[..4].copy_from_slice(b"JUNK");
    let mut wrong_wave = canonical_wave();
    wrong_wave[8..12].copy_from_slice(b"AVI ");

    assert!(!validate(&wrong_riff));
    assert!(!validate(&wrong_wave));
}

#[test]
fn canonical_pcm_wave_requires_format_then_nonempty_aligned_data() {
    let format = chunk(b"fmt ", &pcm_format(2, 48_000, 16));
    let data = chunk(b"data", &[0; 8]);

    assert!(validate(&wave(&[format.clone(), data.clone()])));
    assert!(!validate(&wave(&[data, format.clone()])));
    assert!(!validate(&wave(&[format.clone(), chunk(b"data", &[])])));
    assert!(!validate(&wave(&[format, chunk(b"data", &[0; 3])])));
}

#[test]
fn chunk_boundaries_prevent_fake_format_and_data_markers() {
    let fake_format = chunk(
        b"JUNK",
        b"fmt \x10\0\0\0\x01\0\x01\0\x44\xac\0\0\x88\x58\x01\0\x02\0\x10\0data\x02\0\0\0\0\0",
    );
    let compressed = {
        let mut value = pcm_format(1, 44_100, 16);
        value[0..2].copy_from_slice(&3u16.to_le_bytes());
        chunk(b"fmt ", &value)
    };

    assert!(!validate(&wave(std::slice::from_ref(&fake_format))));
    assert!(!validate(&wave(&[
        fake_format,
        compressed,
        chunk(b"data", &[0; 2]),
    ])));
}

#[test]
fn odd_unknown_chunks_use_declared_padding_without_hiding_following_chunks() {
    let junk = chunk(b"JUNK", b"x");
    let format = chunk(b"fmt ", &pcm_format(1, 44_100, 16));
    let data = chunk(b"data", &[0; 2]);

    assert!(validate(&wave(&[junk, format, data])));
}

#[test]
fn chunk_budget_accepts_the_limit_and_rejects_one_more_chunk() {
    let mut chunks = vec![chunk(b"JUNK", &[]); MAX_WAV_CHUNKS - 2];
    chunks.push(chunk(b"fmt ", &pcm_format(1, 44_100, 16)));
    chunks.push(chunk(b"data", &[0; 2]));

    assert!(validate(&wave(&chunks)));

    chunks.insert(0, chunk(b"JUNK", &[]));
    assert!(!validate(&wave(&chunks)));
}

#[test]
fn pcm_format_bounds_and_derived_rates_must_be_consistent() {
    for invalid in [
        pcm_format(0, 44_100, 16),
        pcm_format(3, 44_100, 16),
        pcm_format(1, 7_999, 16),
        pcm_format(1, 192_001, 16),
        pcm_format(1, 44_100, 12),
    ] {
        assert!(!validate(&wave(&[
            chunk(b"fmt ", &invalid),
            chunk(b"data", &[0; 4]),
        ])));
    }

    let mut wrong_align = pcm_format(1, 44_100, 16);
    wrong_align[12..14].copy_from_slice(&4u16.to_le_bytes());
    assert!(!validate(&wave(&[
        chunk(b"fmt ", &wrong_align),
        chunk(b"data", &[0; 4]),
    ])));

    let mut wrong_rate = pcm_format(1, 44_100, 16);
    wrong_rate[8..12].copy_from_slice(&1u32.to_le_bytes());
    assert!(!validate(&wave(&[
        chunk(b"fmt ", &wrong_rate),
        chunk(b"data", &[0; 4]),
    ])));
}

#[test]
fn riff_length_truncation_duplicate_format_and_extended_format_fail_closed() {
    let format = chunk(b"fmt ", &pcm_format(1, 44_100, 16));
    let data = chunk(b"data", &[0; 2]);
    let mut wrong_length = wave(&[format.clone(), data.clone()]);
    wrong_length[4..8].copy_from_slice(&0u32.to_le_bytes());

    assert!(!validate(&wrong_length));
    assert!(!validate(&wave(&[format.clone(), format, data])));
    assert!(!validate(&wave(&[
        chunk(b"fmt ", &[0; 18]),
        chunk(b"data", &[0; 2]),
    ])));
}
