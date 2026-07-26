use std::path::Path;

use super::{
    classify_installation_channel, parse_exec_start_path, property_value, InstallationChannel,
};

const HOME_UNITS: &str = "/home/test/.config/systemd/user";
const HOME_BIN: &str = "/home/test/.local/bin";

#[test]
fn matching_home_and_system_paths_select_one_installation_channel() {
    assert_eq!(
        classify_installation_channel(
            Path::new("/home/test/.config/systemd/user/unixnotis-daemon.service"),
            Path::new("/home/test/.local/bin/unixnotis-daemon"),
            Path::new(HOME_UNITS),
            Path::new(HOME_BIN),
        ),
        InstallationChannel::HomeLocal
    );
    assert_eq!(
        classify_installation_channel(
            Path::new("/usr/lib/systemd/user/unixnotis-daemon.service"),
            Path::new("/usr/bin/unixnotis-daemon"),
            Path::new(HOME_UNITS),
            Path::new(HOME_BIN),
        ),
        InstallationChannel::SystemPackage
    );
}

#[test]
fn crossed_unit_and_binary_paths_are_always_mixed() {
    for (unit, binary) in [
        (
            "/home/test/.config/systemd/user/unixnotis-daemon.service",
            "/usr/bin/unixnotis-daemon",
        ),
        (
            "/usr/lib/systemd/user/unixnotis-daemon.service",
            "/home/test/.local/bin/unixnotis-daemon",
        ),
    ] {
        assert_eq!(
            classify_installation_channel(
                Path::new(unit),
                Path::new(binary),
                Path::new(HOME_UNITS),
                Path::new(HOME_BIN),
            ),
            InstallationChannel::Mixed
        );
    }
}

#[test]
fn custom_paths_are_not_silently_treated_as_home_or_package_installs() {
    assert_eq!(
        classify_installation_channel(
            Path::new("/opt/systemd/user/unixnotis-daemon.service"),
            Path::new("/opt/unixnotis/bin/unixnotis-daemon"),
            Path::new(HOME_UNITS),
            Path::new(HOME_BIN),
        ),
        InstallationChannel::Unknown
    );
}

#[test]
fn systemd_exec_start_parser_reads_only_the_structured_path_field() {
    assert_eq!(
        parse_exec_start_path(
            "{ path=/home/test/.local/bin/unixnotis-daemon ; argv[]=/home/test/.local/bin/unixnotis-daemon ; ignore_errors=no ; }"
        ),
        Some("/home/test/.local/bin/unixnotis-daemon")
    );
    assert_eq!(parse_exec_start_path("argv[]=/tmp/fake"), None);
}

#[test]
fn systemd_property_parser_requires_an_exact_nonempty_key() {
    let output = "FragmentPath=/home/test/unit\nExecStart={ path=/home/test/bin ; }\n";

    assert_eq!(
        property_value(output, "FragmentPath"),
        Some("/home/test/unit")
    );
    assert_eq!(
        property_value(output, "ExecStart"),
        Some("{ path=/home/test/bin ; }")
    );
    assert_eq!(property_value(output, "Path"), None);
    assert_eq!(property_value("FragmentPath=\n", "FragmentPath"), None);
    assert_eq!(
        property_value("FragmentPathx=/tmp/wrong\n", "FragmentPath"),
        None
    );
}
