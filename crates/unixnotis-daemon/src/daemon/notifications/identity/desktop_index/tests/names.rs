use std::path::Path;

use super::super::is_shared_launcher;

#[test]
fn shared_launchers_are_never_application_specific_associations() {
    for launcher in [
        "/bin/sh",
        "/usr/bin/bash",
        "/usr/bin/env",
        "/usr/bin/python3",
        "/usr/bin/python3.12",
        "/usr/bin/node",
        "/usr/bin/java",
        "/usr/bin/electron",
        "/usr/bin/wine",
        "/usr/bin/flatpak",
        "/usr/bin/gtk-launch",
    ] {
        assert!(
            is_shared_launcher(Path::new(launcher)),
            "{launcher} must not establish application identity"
        );
    }
    assert!(!is_shared_launcher(Path::new("/usr/bin/signal-desktop")));
}
