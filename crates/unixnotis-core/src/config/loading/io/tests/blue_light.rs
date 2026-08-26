//! Behavioral coverage for each shipped blue-light backend

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use super::support::test_root;

const LIBRARY: &str = include_str!("../../../../../assets/scripts/unixnotis-blue-light-lib");

fn write_executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write fake backend");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod fake backend");
}

fn backend_fixture(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = test_root(label);
    let bin = root.join("bin");
    let log = root.join("calls.log");
    fs::create_dir_all(&bin).expect("create fake backend directory");
    fs::write(root.join("blue-light-lib"), LIBRARY).expect("write blue-light library");
    write_executable(&bin.join("nohup"), "#!/bin/sh\nexec \"$@\"\n");
    let logger = "#!/bin/sh\nprintf '%s %s\\n' \"${0##*/}\" \"$*\" >> \"$TEST_LOG\"\n";
    for backend in ["hyprsunset", "gammastep", "wlsunset", "sunsetr"] {
        write_executable(&bin.join(backend), logger);
    }
    (root, log)
}

#[test]
fn every_supported_backend_receives_its_expected_start_arguments() {
    let cases = [
        ("hyprsunset", "hyprsunset --temperature 4500"),
        ("gammastep", "gammastep -m wayland -l 0:0 -t 4500:4500 -P"),
        ("wlsunset", "wlsunset -t 4500 -T 4500 -l 0 -L 0"),
        ("sunsetr", "sunsetr test 4500 90"),
    ];

    for (backend, expected) in cases {
        let (root, log) = backend_fixture(&format!("blue-light-start-{backend}"));
        let status = Command::new("/bin/sh")
            .args([
                "-c",
                ". \"$1\"; start_backend \"$2\"; wait",
                "blue-light-test",
            ])
            .arg(root.join("blue-light-lib"))
            .arg(backend)
            .env("PATH", root.join("bin"))
            .env("TEST_LOG", &log)
            .status()
            .expect("run backend start");

        assert!(status.success(), "backend failed: {backend}");
        assert_eq!(
            fs::read_to_string(&log).expect("read backend log").trim(),
            expected
        );
        let _ = fs::remove_dir_all(root);
    }
}

#[test]
fn stopping_night_mode_visits_every_active_supported_backend() {
    let (root, log) = backend_fixture("blue-light-stop-all");
    write_executable(&root.join("bin/pgrep"), "#!/bin/sh\nexit 0\n");
    write_executable(
        &root.join("bin/pkill"),
        "#!/bin/sh\nprintf 'pkill %s\\n' \"$*\" >> \"$TEST_LOG\"\n",
    );
    write_executable(&root.join("bin/sleep"), "#!/bin/sh\nexit 0\n");

    let status = Command::new("/bin/sh")
        .args(["-c", ". \"$1\"; stop_active_backends", "blue-light-test"])
        .arg(root.join("blue-light-lib"))
        .env("PATH", root.join("bin"))
        .env("TEST_LOG", &log)
        .status()
        .expect("stop every active backend");

    assert!(status.success());
    let calls = fs::read_to_string(&log).expect("read stop calls");
    for expected in [
        "pkill -x hyprsunset",
        "pkill -x gammastep",
        "pkill -x wlsunset",
        "sunsetr stop",
        "pkill -x sunsetr",
    ] {
        assert!(
            calls.lines().any(|call| call == expected),
            "missing {expected}"
        );
    }
    assert!(
        !calls.lines().any(|call| call == "gammastep -x"),
        "stopping Night mode must not invoke the blocking reset process"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn night_mode_falls_through_when_the_first_installed_backend_cannot_stay_running() {
    let (root, log) = backend_fixture("blue-light-fallback");
    let marker = root.join("gammastep.running");
    write_executable(
        &root.join("bin/gammastep"),
        "#!/bin/sh\nprintf '%s %s\\n' \"${0##*/}\" \"$*\" >> \"$TEST_LOG\"\n: > \"$TEST_MARKER\"\n",
    );
    write_executable(
        &root.join("bin/pgrep"),
        "#!/bin/sh\n[ \"$2\" = gammastep ] && [ -f \"$TEST_MARKER\" ]\n",
    );
    write_executable(&root.join("bin/pkill"), "#!/bin/sh\nexit 0\n");
    write_executable(
        &root.join("bin/sleep"),
        "#!/bin/sh\nexec /bin/sleep \"$1\"\n",
    );

    let status = Command::new("/bin/sh")
        .args(["-c", ". \"$1\"; start_available_backend", "blue-light-test"])
        .arg(root.join("blue-light-lib"))
        .env("PATH", root.join("bin"))
        .env("TEST_LOG", &log)
        .env("TEST_MARKER", &marker)
        .env("UNIXNOTIS_BLUE_LIGHT_STARTUP_DELAY", "0.5")
        .status()
        .expect("start first healthy blue-light backend");

    assert!(status.success());
    let calls = fs::read_to_string(&log).expect("read backend calls");
    assert!(calls.lines().any(|call| call.starts_with("hyprsunset ")));
    assert!(calls
        .lines()
        .any(|call| call.starts_with("gammastep -m wayland")));
    assert!(
        !calls.lines().any(|call| call == "gammastep -x"),
        "fallback must not block on an inactive gammastep reset"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn blue_light_scripts_do_not_use_cross_user_temporary_state() {
    for script in crate::DEFAULT_SCRIPTS {
        if script.relative_path.contains("blue-light") {
            assert!(!script.contents.contains("STATE_FILE"));
            assert!(!script.contents.contains("/tmp/unixnotis"));
        }
    }
}
