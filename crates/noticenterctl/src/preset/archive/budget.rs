use std::io::{self, Read};

pub(super) const MAX_PRESET_COMPRESSED_BYTES: u64 = 96 * 1024 * 1024;
pub(super) const MAX_PRESET_DECOMPRESSED_BYTES: u64 = 80 * 1024 * 1024;

pub(super) struct DecompressedBudget<R> {
    inner: R,
    consumed: u64,
    limit: u64,
}

impl<R> DecompressedBudget<R> {
    pub(super) const fn new(inner: R, limit: u64) -> Self {
        Self {
            inner,
            consumed: 0,
            limit,
        }
    }
}

impl<R: Read> Read for DecompressedBudget<R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }

        let remaining = self.limit.saturating_sub(self.consumed);
        if remaining == 0 {
            // Probe one byte so an archive ending exactly at the limit can still reach EOF
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe)? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "preset bundle exceeds the {} byte decompressed limit",
                        self.limit
                    ),
                )),
            };
        }

        // The wrapped decoder never receives a buffer larger than the remaining budget
        let allowed = usize::try_from(remaining)
            .unwrap_or(usize::MAX)
            .min(output.len());
        let read = self.inner.read(&mut output[..allowed])?;
        self.consumed = self
            .consumed
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "archive byte overflow"))?;
        Ok(read)
    }
}
