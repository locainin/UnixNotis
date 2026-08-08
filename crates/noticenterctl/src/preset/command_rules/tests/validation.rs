use super::support::{temp_root, validate_command_paths_in_config_bytes};

#[test]
fn validation_rejects_ld_preload_path_that_leaves_root() {
    let config_dir = temp_root("ld-preload-outside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=/tmp/evil.so /bin/true\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject LD_PRELOAD outside config root");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_rejects_quoted_ld_preload_paths_that_leave_root() {
    let config_dir = temp_root("quoted-ld-preload-outside");
    for command in [
        "LD_PRELOAD=\"/tmp/evil.so\" /bin/true",
        "LD_PRELOAD='/tmp/evil.so' /bin/true",
        "env LD_PRELOAD=/tmp/evil.so /bin/true",
    ] {
        let config = format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n"
        );
        validate_command_paths_in_config_bytes(
            &config_dir,
            config.as_bytes(),
            "preset import blocked",
        )
        .expect_err("reject quoted or env-wrapped preload escape");
    }
}

#[test]
fn validation_migrates_tilde_syntax_to_shell_and_rejects_malformed_quoting() {
    let config_dir = temp_root("tilde-and-quote");
    let tilde = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"~/outside-script\"\n";
    validate_command_paths_in_config_bytes(&config_dir, tilde, "preset import blocked")
        .expect("tilde syntax is an explicit shell command after migration");

    let malformed = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = 'echo \"unterminated'\n";
    validate_command_paths_in_config_bytes(&config_dir, malformed, "preset import blocked")
        .expect_err("reject malformed command quoting");
}

#[test]
fn validation_rejects_home_override_and_env_wrapped_absolute_program() {
    let config_dir = temp_root("home-and-env-program");
    for command in ["HOME=/tmp ./script", "env SAFE=value /bin/true"] {
        let config = format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n"
        );
        validate_command_paths_in_config_bytes(
            &config_dir,
            config.as_bytes(),
            "preset import blocked",
        )
        .expect_err("reject path policy escape");
    }
}

#[test]
fn validation_rejects_space_separated_ld_preload_path_that_leaves_root() {
    let config_dir = temp_root("space-separated-ld-preload");
    let inside = config_dir.join("libsafe.so");
    let command = format!(
        "LD_PRELOAD='{} /tmp/libevil.so' scripts/probe",
        inside.display()
    );
    let config = format!(
        "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n"
    );

    let error = validate_command_paths_in_config_bytes(
        &config_dir,
        config.as_bytes(),
        "preset import blocked",
    )
    .expect_err("reject second preload object outside config root");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_rejects_semicolon_separated_library_directory_that_leaves_root() {
    let config_dir = temp_root("semicolon-library-path");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_LIBRARY_PATH='lib;/tmp/evil' scripts/probe\"\n";

    validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
        .expect_err("reject semicolon-separated loader directory outside config root");
}

#[test]
fn validation_accepts_empty_list_components_with_the_pinned_config_cwd() {
    let config_dir = temp_root("empty-loader-component");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_LIBRARY_PATH=':lib;' PATH=:bin scripts/probe\"\n";

    validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
        .expect("empty path components should resolve to the pinned config cwd");
}

#[test]
fn validation_keeps_single_path_environment_values_unsplit() {
    let config_dir = temp_root("single-path-colon");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"HOME=profiles/home:secondary BASH_ENV=scripts/start:up scripts/probe\"\n";

    validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
        .expect("single path values containing colons should remain one relative path");
}

#[test]
fn validation_rejects_pythonhome_exec_prefix_outside_root() {
    let config_dir = temp_root("pythonhome-outside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"PYTHONHOME='runtime:/tmp/outside' python3 -c pass\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject external Python exec prefix");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_accepts_pythonhome_single_and_relative_prefix_pair() {
    let config_dir = temp_root("pythonhome-relative");
    for command in [
        "PYTHONHOME=runtime python3 -c pass",
        "PYTHONHOME='runtime:exec-runtime' python3 -c pass",
    ] {
        let config = format!(
            "[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = {command:?}\n"
        );

        validate_command_paths_in_config_bytes(
            &config_dir,
            config.as_bytes(),
            "preset import blocked",
        )
        .unwrap_or_else(|error| panic!("valid PYTHONHOME was rejected for {command}: {error}"));
    }
}

#[test]
fn validation_rejects_bare_library_names_with_ambiguous_loader_search() {
    let config_dir = temp_root("bare-env-token");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=libprobe.so scripts/probe\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject loader object without an explicit path");

    assert!(error
        .to_string()
        .contains("unsafe environment path semantics"));
}

#[test]
fn validation_rejects_colon_separated_env_path_that_leaves_root() {
    let config_dir = temp_root("pythonpath-outside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.cards]]\nlabel = \"Probe\"\ncmd = \"PYTHONPATH=scripts:/tmp/evil python3 -c pass\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject PYTHONPATH outside config root");

    assert!(error
        .to_string()
        .contains("resolves outside the UnixNotis config directory"));
}

#[test]
fn validation_accepts_dangerous_env_paths_inside_root() {
    let config_dir = temp_root("env-path-inside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=scripts/libprobe.so scripts/probe\"\n";

    validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
        .expect("config-root-relative env paths should be allowed");
}
