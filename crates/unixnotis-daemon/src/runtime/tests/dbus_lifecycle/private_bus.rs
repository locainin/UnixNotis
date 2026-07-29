use std::io::{self, BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, RecvTimeoutError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BUS_READY_TIMEOUT: Duration = Duration::from_secs(2);
const MAX_BUS_ADDRESS_BYTES: usize = 4 * 1024;

// Parallel lifecycle tests need independent socket directories
static NEXT_BUS: AtomicUsize = AtomicUsize::new(0);

pub(super) struct PrivateBus {
    child: Child,
    socket: PathBuf,
    pub(super) address: String,
}

impl PrivateBus {
    pub(super) fn start() -> Self {
        let socket = bus_socket();
        let listen_address = format!("unix:path={}", socket.display());

        // Resolve from protected roots because tests may temporarily replace PATH
        let daemon = unixnotis_core::util::trusted_system_program_path("dbus-daemon")
            .expect("find dbus-daemon in a trusted system directory");
        let mut child = Command::new(daemon)
            .args([
                "--session",
                "--nofork",
                "--nopidfile",
                "--nosyslog",
                "--print-address=1",
                &format!("--address={listen_address}"),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start private D-Bus session bus");

        // The first output line proves that the requested listener is ready
        let stdout = child.stdout.take().expect("capture private bus address");
        let address = read_bus_address(&mut child, stdout, &listen_address)
            .expect("read private D-Bus session bus address promptly");

        Self {
            child,
            socket,
            address,
        }
    }

    pub(super) fn terminate(&mut self) {
        // Reaping the daemon prevents process and socket leaks between tests
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        self.terminate();
        let _ = std::fs::remove_file(&self.socket);
        if let Some(parent) = self.socket.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

fn read_bus_address(
    child: &mut Child,
    stdout: ChildStdout,
    expected_prefix: &str,
) -> io::Result<String> {
    let (sender, receiver) = sync_channel(1);

    // A worker keeps the pipe read from blocking the test indefinitely
    std::thread::spawn(move || {
        let mut address = String::new();
        let limit = u64::try_from(MAX_BUS_ADDRESS_BYTES + 1)
            .expect("private bus address limit should fit in u64");
        let result = BufReader::new(stdout)
            .take(limit)
            .read_line(&mut address)
            .and_then(|read| validate_address_line(read, address));
        let _ = sender.send(result);
    });

    let result = match receiver.recv_timeout(BUS_READY_TIMEOUT) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "private D-Bus session bus did not report its address promptly",
        )),
        Err(RecvTimeoutError::Disconnected) => Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "private D-Bus address reader stopped unexpectedly",
        )),
    }
    .and_then(|address| validate_listener(address, expected_prefix));

    if result.is_err() {
        // Startup failures occur before a guard exists, so cleanup happens here
        let _ = child.kill();
        let _ = child.wait();
    }

    result
}

fn validate_address_line(read: usize, address: String) -> io::Result<String> {
    if read == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "private D-Bus session bus closed before reporting its address",
        ));
    }
    if read > MAX_BUS_ADDRESS_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "private D-Bus session bus address exceeded the test limit",
        ));
    }
    if !address.ends_with('\n') {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "private D-Bus session bus address did not end with a newline",
        ));
    }

    Ok(address.trim().to_string())
}

fn validate_listener(address: String, expected_prefix: &str) -> io::Result<String> {
    if address.starts_with(expected_prefix) {
        Ok(address)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "private D-Bus session bus returned an unexpected address",
        ))
    }
}

fn bus_socket() -> PathBuf {
    // Time, process, and serial values keep concurrent test roots independent
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must be after the Unix epoch")
        .as_nanos();
    let serial = NEXT_BUS.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "unixnotis-runtime-dbus-{}-{stamp}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create private D-Bus directory");
    root.join("bus.sock")
}
