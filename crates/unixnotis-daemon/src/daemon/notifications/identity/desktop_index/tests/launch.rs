use std::fs;
use std::path::Path;

use super::super::launch::{
    build_launch_spec, field_value_matches, launch_spec_matches_sender,
    MAX_EXEC_TEMPLATE_ARGUMENTS, MAX_EXEC_TEMPLATE_BYTES, MAX_PROCESS_ARGUMENTS,
};
use super::super::model::{FieldCode, LaunchArgument, LaunchSpec};
use crate::daemon::notifications::identity::executable::executable_evidence_for_path;
use crate::test_support::TempRoot;

#[test]
fn fixed_immutable_application_argument_is_matched_exactly() {
    let shell = executable_evidence_for_path(Path::new("/usr/bin/sh")).expect("system shell");
    let immutable_script =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system fixture");
    assert!(immutable_script.identity.is_system_managed());

    let root = TempRoot::new("launch-spec-shared-runtime");
    let path = root.join("org.example.Script.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=Script\nExec=/usr/bin/sh /usr/bin/true %U\n",
    )
    .expect("write desktop entry");
    let desktop = gio::DesktopAppInfo::from_filename(&path).expect("parse desktop entry");
    let (_executable_path, spec) = build_launch_spec(&desktop, &path).expect("build launch spec");

    assert!(launch_spec_matches_sender(
        &spec,
        shell.identity,
        &[
            b"/usr/bin/sh".to_vec(),
            b"/usr/bin/true".to_vec(),
            b"file:///tmp/input".to_vec(),
        ],
    ));
    assert!(!launch_spec_matches_sender(
        &spec,
        shell.identity,
        &[
            b"/usr/bin/sh".to_vec(),
            b"/tmp/fake-script".to_vec(),
            b"file:///tmp/input".to_vec(),
        ],
    ));
}

#[test]
fn user_writable_literal_payload_cannot_support_a_system_association() {
    let root = TempRoot::new("launch-spec-user-payload");
    let payload = root.join("application-script");
    fs::write(&payload, "exit 0\n").expect("write user payload");
    let desktop_path = root.join("org.example.UserPayload.desktop");
    fs::write(
        &desktop_path,
        format!(
            "[Desktop Entry]\nType=Application\nName=User Payload\nExec=/usr/bin/sh {}\n",
            payload.display()
        ),
    )
    .expect("write desktop entry");
    let desktop = gio::DesktopAppInfo::from_filename(&desktop_path).expect("parse desktop entry");

    let (_executable_path, spec) =
        build_launch_spec(&desktop, &desktop_path).expect("build launch spec");

    assert!(!spec.literal_files_are_system_managed);
}

#[test]
fn launch_spec_rejects_unmodeled_flags_and_invalid_url_fields() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system fixture");
    let root = TempRoot::new("launch-spec-fields");
    let path = root.join("org.example.True.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=True\nExec=/usr/bin/true --fixed %u\n",
    )
    .expect("write desktop entry");
    let desktop = gio::DesktopAppInfo::from_filename(&path).expect("parse desktop entry");
    let (_executable_path, spec) = build_launch_spec(&desktop, &path).expect("build launch spec");

    assert!(launch_spec_matches_sender(
        &spec,
        executable.identity,
        &[
            b"/usr/bin/true".to_vec(),
            b"--fixed".to_vec(),
            b"https://example.invalid/item".to_vec(),
        ],
    ));
    assert!(!launch_spec_matches_sender(
        &spec,
        executable.identity,
        &[
            b"/usr/bin/true".to_vec(),
            b"--unexpected".to_vec(),
            b"https://example.invalid/item".to_vec(),
        ],
    ));
    assert!(!launch_spec_matches_sender(
        &spec,
        executable.identity,
        &[
            b"/usr/bin/true".to_vec(),
            b"--fixed".to_vec(),
            b"--not-a-url".to_vec(),
        ],
    ));
}

#[test]
fn launch_spec_enforces_template_size_and_argument_limits_at_the_boundary() {
    let root = TempRoot::new("launch-spec-limits");
    let executable_prefix = "/usr/bin/true ";

    for (name, template, accepted) in [
        (
            "exact-bytes",
            format!(
                "{executable_prefix}{}",
                "x".repeat(MAX_EXEC_TEMPLATE_BYTES - executable_prefix.len())
            ),
            true,
        ),
        (
            "too-many-bytes",
            format!(
                "{executable_prefix}{}",
                "x".repeat(MAX_EXEC_TEMPLATE_BYTES + 1 - executable_prefix.len())
            ),
            false,
        ),
        (
            "exact-arguments",
            std::iter::once("/usr/bin/true")
                .chain(std::iter::repeat_n("x", MAX_EXEC_TEMPLATE_ARGUMENTS - 1))
                .collect::<Vec<_>>()
                .join(" "),
            true,
        ),
        (
            "too-many-arguments",
            std::iter::once("/usr/bin/true")
                .chain(std::iter::repeat_n("x", MAX_EXEC_TEMPLATE_ARGUMENTS))
                .collect::<Vec<_>>()
                .join(" "),
            false,
        ),
    ] {
        let path = root.join(format!("{name}.desktop"));
        fs::write(
            &path,
            format!("[Desktop Entry]\nType=Application\nName=Limits\nExec={template}\n"),
        )
        .expect("write boundary desktop entry");
        let desktop = gio::DesktopAppInfo::from_filename(&path)
            .unwrap_or_else(|| panic!("parse {name} desktop entry"));

        assert_eq!(
            build_launch_spec(&desktop, &path).is_some(),
            accepted,
            "{name}"
        );
    }
}

#[test]
fn launch_spec_parses_every_supported_desktop_field_code() {
    let root = TempRoot::new("launch-spec-field-codes");
    let path = root.join("org.example.Fields.desktop");
    fs::write(
        &path,
        "[Desktop Entry]\nType=Application\nName=Fields\nIcon=field-icon\nExec=/usr/bin/true %f %F %u %U %c %k %i\n",
    )
    .expect("write field-code desktop entry");
    let desktop = gio::DesktopAppInfo::from_filename(&path).expect("parse desktop entry");
    let (_executable_path, spec) = build_launch_spec(&desktop, &path).expect("build launch spec");

    assert!(matches!(
        spec.arguments[0],
        LaunchArgument::FieldCode(FieldCode::File)
    ));
    assert!(matches!(
        spec.arguments[1],
        LaunchArgument::FieldCode(FieldCode::Files)
    ));
    assert!(matches!(
        spec.arguments[2],
        LaunchArgument::FieldCode(FieldCode::Url)
    ));
    assert!(matches!(
        spec.arguments[3],
        LaunchArgument::FieldCode(FieldCode::Urls)
    ));
    assert!(matches!(
        &spec.arguments[4],
        LaunchArgument::Literal(argument) if argument.value == b"Fields"
    ));
    assert!(matches!(
        &spec.arguments[5],
        LaunchArgument::Literal(argument)
            if argument.value == path.as_os_str().as_encoded_bytes()
    ));
    assert!(matches!(
        &spec.arguments[6],
        LaunchArgument::OptionalIcon { name } if name == "field-icon"
    ));
}

#[test]
fn process_matcher_checks_identity_emptiness_and_argument_limits_independently() {
    let executable =
        executable_evidence_for_path(Path::new("/usr/bin/true")).expect("system fixture");
    let other = executable_evidence_for_path(Path::new("/usr/bin/false")).expect("other fixture");
    let spec = LaunchSpec {
        executable: executable.identity,
        arguments: vec![LaunchArgument::FieldCode(FieldCode::Files)],
        environment: Vec::new(),
        wrappers: Vec::new(),
        literal_files_are_system_managed: true,
    };
    let exact_limit =
        std::iter::repeat_n(b"input".to_vec(), MAX_PROCESS_ARGUMENTS).collect::<Vec<_>>();
    let over_limit =
        std::iter::repeat_n(b"input".to_vec(), MAX_PROCESS_ARGUMENTS + 1).collect::<Vec<_>>();

    assert!(launch_spec_matches_sender(
        &spec,
        executable.identity,
        &exact_limit
    ));
    assert!(!launch_spec_matches_sender(
        &spec,
        other.identity,
        &exact_limit
    ));
    assert!(!launch_spec_matches_sender(&spec, executable.identity, &[]));
    assert!(!launch_spec_matches_sender(
        &spec,
        executable.identity,
        &over_limit
    ));
}

#[test]
fn field_values_reject_empty_and_option_shaped_arguments_independently() {
    assert!(!field_value_matches(FieldCode::File, b""));
    assert!(!field_value_matches(FieldCode::Files, b"--option"));
    assert!(field_value_matches(FieldCode::File, b"relative-file"));
    assert!(field_value_matches(
        FieldCode::Url,
        b"https://example.invalid/item"
    ));
}
