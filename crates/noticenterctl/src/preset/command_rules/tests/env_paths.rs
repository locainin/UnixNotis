use std::path::PathBuf;

use super::super::tokens::collect_outside_env_path_tokens;
use super::super::validate_command_paths_in_config_bytes;
use super::temp_root;

#[test]
fn validation_rejects_ld_preload_path_that_leaves_root() {
    let config_dir = temp_root("ld-preload-outside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=/tmp/evil.so /bin/true\"\n";

    let error =
        validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
            .expect_err("reject LD_PRELOAD outside config root");

    assert!(error
        .to_string()
        .contains("points outside the UnixNotis config directory"));
}

#[test]
fn env_path_token_collector_finds_ld_preload_outside_root() {
    let config_dir = temp_root("ld-preload-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=/tmp/evil.so /bin/true");

    assert_eq!(outside.len(), 1);
    assert_eq!(outside[0].0, "LD_PRELOAD");
    assert_eq!(outside[0].1, PathBuf::from("/tmp/evil.so"));
}

#[test]
fn env_path_token_collector_ignores_invalid_env_assignment_names() {
    let config_dir = temp_root("invalid-env-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "/tmp/with=equals /bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_commands_with_carriage_returns() {
    let config_dir = temp_root("carriage-return-env-token");

    let outside =
        collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=/tmp/evil.so\r/bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_unknown_env_names() {
    let config_dir = temp_root("unknown-env-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "WIDGET_DATA=/tmp/evil /bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_complex_shell_commands() {
    let config_dir = temp_root("complex-env-token");

    let outside =
        collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=/tmp/evil.so; /bin/true");

    assert!(outside.is_empty());
}

#[test]
fn env_path_token_collector_ignores_bare_library_names() {
    let config_dir = temp_root("bare-env-token");

    let outside = collect_outside_env_path_tokens(&config_dir, "LD_PRELOAD=libprobe.so /bin/true");

    assert!(outside.is_empty());
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
        .contains("points outside the UnixNotis config directory"));
}

#[test]
fn validation_accepts_dangerous_env_paths_inside_root() {
    let config_dir = temp_root("env-path-inside");
    let config = b"[theme]\nbase_css = \"base.css\"\n[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"LD_PRELOAD=scripts/libprobe.so scripts/probe\"\n";

    validate_command_paths_in_config_bytes(&config_dir, config, "preset import blocked")
        .expect("config-root-relative env paths should be allowed");
}
