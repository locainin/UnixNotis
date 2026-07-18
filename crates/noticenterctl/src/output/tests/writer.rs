use std::io::{self, Write};

use super::write_all_and_flush;

struct FailingWriter {
    kind: io::ErrorKind,
}

impl Write for FailingWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(self.kind, "fixture failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::new(self.kind, "fixture failure"))
    }
}

#[test]
fn output_writer_treats_broken_pipe_as_normal_completion() {
    let mut writer = FailingWriter {
        kind: io::ErrorKind::BrokenPipe,
    };

    write_all_and_flush(&mut writer, b"output").expect("ignore closed pipeline consumer");
}

#[test]
fn output_writer_preserves_real_stream_failures() {
    let mut writer = FailingWriter {
        kind: io::ErrorKind::Other,
    };

    let error = write_all_and_flush(&mut writer, b"output")
        .expect_err("non-pipe output failure must remain visible");

    assert_eq!(error.kind(), io::ErrorKind::Other);
}
