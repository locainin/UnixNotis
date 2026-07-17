//! Consistent handling for command-line output streams

use anyhow::{Context, Result};
use std::io::{self, Write};

pub fn write_stdout(text: &str) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    write_all_and_flush(&mut stdout, text.as_bytes()).context("write command output")
}

pub fn write_stderr(text: &str) -> Result<()> {
    let stderr = io::stderr();
    let mut stderr = stderr.lock();
    write_all_and_flush(&mut stderr, text.as_bytes()).context("write command diagnostic")
}

fn write_all_and_flush(writer: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if let Err(error) = writer.write_all(bytes) {
        return ignore_broken_pipe(error);
    }
    writer.flush().or_else(ignore_broken_pipe)
}

fn ignore_broken_pipe(error: io::Error) -> io::Result<()> {
    // Closed pipeline consumers are a normal CLI termination condition
    if error.kind() == io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(error)
    }
}

#[cfg(test)]
#[path = "tests/writer.rs"]
mod tests;
