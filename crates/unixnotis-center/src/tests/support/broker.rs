use std::io::{self, BufRead, BufReader, Read};
use std::process::{Child, ChildStdout};
use std::sync::mpsc::{sync_channel, RecvTimeoutError};
use std::time::Duration;

const BROKER_READY_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BROKER_ADDRESS_BYTES: usize = 4 * 1024;

pub fn read_broker_address(
    child: &mut Child,
    stdout: ChildStdout,
    expected_prefix: &str,
) -> io::Result<String> {
    let (sender, receiver) = sync_channel(1);

    // The worker isolates the one blocking pipe operation behind a bounded receive
    std::thread::spawn(move || {
        let mut address = String::new();
        let limit = u64::try_from(MAX_BROKER_ADDRESS_BYTES + 1)
            .expect("broker address limit should fit in u64");
        let result = BufReader::new(stdout)
            .take(limit)
            .read_line(&mut address)
            .and_then(|read| validate_line(read, address));
        let _ = sender.send(result);
    });

    let result = match receiver.recv_timeout(BROKER_READY_TIMEOUT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "private broker did not report its address promptly",
        )),
        Err(RecvTimeoutError::Disconnected) => Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "private broker address reader stopped unexpectedly",
        )),
    }
    .and_then(|address| validate_address(address, expected_prefix));

    if result.is_err() {
        // Construction has not returned a guard yet, so failed startup is cleaned up here
        let _ = child.kill();
        let _ = child.wait();
    }

    result
}

fn validate_line(read: usize, address: String) -> io::Result<String> {
    if read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "private broker closed stdout before reporting its address",
        ));
    }
    if read > MAX_BROKER_ADDRESS_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "private broker address exceeded the test limit",
        ));
    }
    if !address.ends_with('\n') {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "private broker address did not end with a newline",
        ));
    }

    Ok(address.trim().to_string())
}

fn validate_address(address: String, expected_prefix: &str) -> io::Result<String> {
    // The fixture must connect to the exact private socket it just created
    if address.starts_with(expected_prefix) {
        Ok(address)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "private broker returned an unexpected address",
        ))
    }
}
