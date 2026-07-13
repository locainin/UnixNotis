use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use tar::{EntryType, Header};

static TEST_TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

pub(super) struct TempDirGuard {
    pub(super) path: PathBuf,
}

impl TempDirGuard {
    pub(super) fn new(name: &str) -> Self {
        // Unique temp roots keep tests isolated even when cargo runs them in parallel
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock moved backwards")
            .as_nanos();
        let serial = TEST_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("unixnotis-preset-archive-{name}-{stamp}-{serial}"));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }

    pub(super) fn write(&self, relative_path: &str, contents: &str) -> PathBuf {
        // Test helpers build small fake config trees without touching the real config root
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, contents).expect("write file");
        path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub(super) fn write_raw_gzip_tar(
    bundle_path: &Path,
    write_entries: impl FnOnce(&mut GzEncoder<fs::File>),
) {
    let output = fs::File::create(bundle_path).expect("create raw bundle");
    let mut encoder = GzEncoder::new(output, Compression::default());
    write_entries(&mut encoder);
    // Two zero blocks mark the end of the tar stream
    encoder.write_all(&[0_u8; 1024]).expect("write tar eof");
    let file = encoder.finish().expect("finish gzip");
    file.sync_all().expect("sync raw bundle");
}

pub(super) fn append_raw_tar_file(
    encoder: &mut GzEncoder<fs::File>,
    path: &Path,
    contents: &[u8],
    mode: u32,
) {
    append_raw_tar_header(encoder, path, contents.len() as u64, mode);
    encoder.write_all(contents).expect("write tar file body");
    let padding = (512 - (contents.len() % 512)) % 512;
    if padding > 0 {
        encoder
            .write_all(&vec![0_u8; padding])
            .expect("write tar file padding");
    }
}

pub(super) fn append_raw_tar_header(
    encoder: &mut GzEncoder<fs::File>,
    path: &Path,
    size: u64,
    mode: u32,
) {
    let mut header = Header::new_gnu();
    header.set_path(path).expect("set raw tar path");
    header.set_mode(mode);
    header.set_size(size);
    header.set_cksum();
    encoder
        .write_all(header.as_bytes())
        .expect("write raw tar header");
}

pub(super) fn append_raw_tar_dir(encoder: &mut GzEncoder<fs::File>, path: &Path) {
    let mut header = Header::new_gnu();
    header.set_path(path).expect("set raw tar dir path");
    header.set_entry_type(EntryType::Directory);
    header.set_mode(0o755);
    header.set_size(0);
    header.set_cksum();
    encoder
        .write_all(header.as_bytes())
        .expect("write raw tar dir header");
}
