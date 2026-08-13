//! Shell-launcher syntax acceptance and rejection cases

use std::path::Path;

use super::super::syntax::literal_final_exec_target;

#[test]
fn package_shell_launcher_extracts_literal_final_target() {
    let source = b"#!/bin/sh\nexec /usr/lib/example/example \"$@\"\n";

    assert_eq!(
        literal_final_exec_target(source).as_deref(),
        Some(Path::new("/usr/lib/example/example"))
    );
}

#[test]
fn package_shell_launcher_allows_dynamic_arguments_after_literal_target() {
    let source = br#"#!/usr/bin/env bash

FLAGS_FILE="${XDG_CONFIG_HOME:-$HOME/.config}/example-flags.conf"
if [[ -f "${FLAGS_FILE}" ]]; then
    FLAGS="$(sed 's/#.*//' "${FLAGS_FILE}" | tr '\n' ' ')"
fi
exec /usr/lib/example/example $FLAGS "$@"
"#;

    assert_eq!(
        literal_final_exec_target(source).as_deref(),
        Some(Path::new("/usr/lib/example/example"))
    );
}

#[test]
fn dynamic_exec_targets_are_rejected() {
    for source in [
        b"#!/bin/sh\nexec \"$TARGET\" \"$@\"\n".as_slice(),
        b"#!/bin/sh\nexec ${TARGET} \"$@\"\n".as_slice(),
        b"#!/bin/sh\nexec \"$(find-runtime)\" \"$@\"\n".as_slice(),
    ] {
        assert!(
            literal_final_exec_target(source).is_none(),
            "dynamic executable target must fail closed"
        );
    }
}

#[test]
fn relative_exec_targets_are_rejected() {
    for source in [
        b"#!/bin/sh\nexec ./example \"$@\"\n".as_slice(),
        b"#!/bin/sh\nexec example \"$@\"\n".as_slice(),
        b"#!/bin/sh\nexec /usr/lib/../bin/example \"$@\"\n".as_slice(),
    ] {
        assert!(
            literal_final_exec_target(source).is_none(),
            "relative or normalized executable target must fail closed"
        );
    }
}

#[test]
fn multiple_exec_targets_are_rejected() {
    let source = b"#!/bin/sh\nexec /usr/lib/example/first || exec /usr/lib/example/second\n";

    assert!(literal_final_exec_target(source).is_none());
}

#[test]
fn exec_with_control_operator_is_rejected() {
    for source in [
        b"#!/bin/sh\nexec /usr/lib/example/example; fallback\n".as_slice(),
        b"#!/bin/sh\nexec /usr/lib/example/example | other\n".as_slice(),
    ] {
        assert!(
            literal_final_exec_target(source).is_none(),
            "control operators around the authoritative exec must fail closed"
        );
    }
}

#[test]
fn sourced_launchers_are_rejected() {
    for command in ["source helper-script", ". helper-script"] {
        let source = format!("#!/bin/sh\n{command}\nexec /usr/lib/example/example \"$@\"\n");
        assert!(
            literal_final_exec_target(source.as_bytes()).is_none(),
            "sourced code can change final command meaning"
        );
    }
}

#[test]
fn commands_after_exec_are_rejected() {
    let source = b"#!/bin/sh\nexec /usr/lib/example/example \"$@\"\necho unreachable\n";

    assert!(literal_final_exec_target(source).is_none());
}

#[test]
fn unsupported_shell_shebang_is_rejected() {
    let source = b"#!/usr/bin/fish\nexec /usr/lib/example/example $argv\n";

    assert!(literal_final_exec_target(source).is_none());
}

#[test]
fn malformed_shell_syntax_is_rejected_even_with_a_literal_final_exec() {
    let source = b"#!/bin/sh\nif then\nexec /usr/lib/example/example \"$@\"\n";

    assert!(literal_final_exec_target(source).is_none());
}

#[test]
fn exec_argument_limit_accepts_the_boundary_and_rejects_one_more() {
    let command = |extra_arguments: usize| {
        format!(
            "#!/bin/sh\nexec /usr/lib/example/example {}\n",
            std::iter::repeat_n("$ARG", extra_arguments)
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    let exact = command(127);
    let over = command(128);

    assert_eq!(
        literal_final_exec_target(exact.as_bytes()).as_deref(),
        Some(Path::new("/usr/lib/example/example"))
    );
    assert!(literal_final_exec_target(over.as_bytes()).is_none());
}

#[test]
fn exec_without_a_target_is_rejected() {
    assert!(literal_final_exec_target(b"#!/bin/sh\nexec\n").is_none());
}
