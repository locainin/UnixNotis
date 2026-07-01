use crate::{Args, RestoreStrategy};

pub(super) fn default_args() -> Args {
    Args {
        config: None,
        trial: false,
        restore: RestoreStrategy::Auto,
        yes: false,
        restore_wait_ms: 2_000,
        check: false,
        run_seconds: None,
    }
}

#[cfg(unix)]
pub(super) fn bind_wayland_socket(path: &std::path::Path) -> std::os::unix::net::UnixListener {
    // Holding the listener keeps the filesystem entry as a real socket for metadata checks
    std::os::unix::net::UnixListener::bind(path).expect("bind wayland socket")
}
