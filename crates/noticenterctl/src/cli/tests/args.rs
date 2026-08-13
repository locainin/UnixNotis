use clap::Parser;
use unixnotis_core::{PanelDebugLevel, INHIBIT_SCOPE_ALL, INHIBIT_SCOPE_POPUPS};

use super::super::args::DebugLevelArg;
use super::super::{
    Args, Command, DevCommand, DndState, DoctorCommand, DoctorServiceManagerArg, InhibitScopeArg,
    PresetCommand,
};

#[test]
fn parses_normal_open_panel_without_developer_options() {
    let args = Args::try_parse_from(["noticenterctl", "open-panel"]).expect("parse open panel");
    assert!(matches!(args.command, Command::OpenPanel));
}

#[test]
fn parses_every_supported_dev_command() {
    for level in ["critical", "warn", "info", "verbose"] {
        let args = Args::try_parse_from(["noticenterctl", "dev", "open-panel", "--level", level])
            .expect("parse developer panel command");
        assert!(matches!(
            args.command,
            Command::Dev {
                command: DevCommand::OpenPanel { .. }
            }
        ));
    }

    for arguments in [
        vec!["noticenterctl", "dev", "refresh-applications"],
        vec!["noticenterctl", "dev", "explain-notification", "42"],
        vec!["noticenterctl", "dev", "dump-active"],
        vec!["noticenterctl", "dev", "dump-history"],
        vec!["noticenterctl", "dev", "logs"],
    ] {
        Args::try_parse_from(arguments).expect("developer command should parse");
    }
}

#[test]
fn dev_open_panel_defaults_to_info() {
    let args = Args::try_parse_from(["noticenterctl", "dev", "open-panel"])
        .expect("parse default developer panel command");
    assert!(matches!(
        args.command,
        Command::Dev {
            command: DevCommand::OpenPanel {
                level: DebugLevelArg::Info
            }
        }
    ));
}

#[test]
fn removed_root_interfaces_are_rejected_without_compatibility_aliases() {
    for arguments in [
        vec!["noticenterctl", "clear-all"],
        vec!["noticenterctl", "refresh-applications"],
        vec!["noticenterctl", "explain-notification", "42"],
        vec!["noticenterctl", "sync-session-environment"],
        vec!["noticenterctl", "open-panel", "--debug"],
        vec!["noticenterctl", "list-active", "--full"],
        vec!["noticenterctl", "list-history", "--full"],
    ] {
        assert!(
            Args::try_parse_from(arguments.clone()).is_err(),
            "removed interface unexpectedly parsed: {arguments:?}"
        );
    }
}

#[test]
fn parses_dnd_toggle() {
    // Confirms the value enum accepts the toggle state for DND commands
    let args = Args::try_parse_from(["noticenterctl", "dnd", "toggle"]).expect("parse args");
    match args.command {
        Command::Dnd {
            state,
            for_duration,
            until,
        } => {
            assert!(matches!(state, DndState::Toggle));
            assert!(for_duration.is_none());
            assert!(until.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_timed_dnd_duration_and_clock_deadline() {
    let duration =
        Args::try_parse_from(["noticenterctl", "dnd", "on", "--for", "30m"]).expect("duration");
    assert!(matches!(
        duration.command,
        Command::Dnd {
            state: DndState::On,
            for_duration: Some(_),
            until: None,
        }
    ));

    let until =
        Args::try_parse_from(["noticenterctl", "dnd", "on", "--until", "08:00"]).expect("clock");
    assert!(matches!(
        until.command,
        Command::Dnd {
            state: DndState::On,
            for_duration: None,
            until: Some(_),
        }
    ));
}

#[test]
fn timed_dnd_options_conflict_and_require_on_state_semantically() {
    assert!(Args::try_parse_from([
        "noticenterctl",
        "dnd",
        "on",
        "--for",
        "30m",
        "--until",
        "08:00"
    ])
    .is_err());

    let command = Args::try_parse_from(["noticenterctl", "dnd", "off", "--for", "30m"])
        .expect("syntax should parse")
        .command;
    assert!(command.validate().is_err());
}

#[test]
fn parses_explicit_clear_variants() {
    for (name, expected) in [
        ("clear", "clear"),
        ("clear-active", "clear-active"),
        ("clear-history", "clear-history"),
    ] {
        let args = Args::try_parse_from(["noticenterctl", name]).expect("parse args");
        match (args.command, expected) {
            (Command::Clear, "clear")
            | (Command::ClearActive, "clear-active")
            | (Command::ClearHistory, "clear-history") => {}
            (other, _) => panic!("unexpected command: {other:?}"),
        }
    }
}

#[test]
fn parses_inhibit_default_scope() {
    // Ensures inhibit defaults to the "all" scope when omitted
    let args = Args::try_parse_from(["noticenterctl", "inhibit", "focus"]).expect("parse args");
    match args.command {
        Command::Inhibit { scope, .. } => {
            assert!(matches!(scope, InhibitScopeArg::All));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_inhibit_popups_scope() {
    // Confirms popups scope is accepted for inhibit calls
    let args = Args::try_parse_from(["noticenterctl", "inhibit", "focus", "--scope", "popups"])
        .expect("parse args");
    match args.command {
        Command::Inhibit { scope, .. } => {
            assert!(matches!(scope, InhibitScopeArg::Popups));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_preset_export_with_repeated_except() {
    // Repeated --except flags should preserve order for later filtering
    let args = Args::try_parse_from([
        "noticenterctl",
        "preset",
        "export",
        "bundle.unixnotis",
        "--except",
        "installer.toml",
        "--except",
        "assets/bg.png",
    ])
    .expect("parse args");
    match args.command {
        Command::Preset {
            command:
                PresetCommand::Export {
                    output,
                    except,
                    force,
                },
        } => {
            assert_eq!(output, "bundle.unixnotis");
            assert_eq!(except, vec!["installer.toml", "assets/bg.png"]);
            assert!(!force);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_preset_import_dry_run() {
    // Dry-run import should parse without touching D-Bus
    let args = Args::try_parse_from([
        "noticenterctl",
        "preset",
        "import",
        "bundle.unixnotis",
        "--dry-run",
    ])
    .expect("parse args");
    match args.command {
        Command::Preset {
            command:
                PresetCommand::Import {
                    input,
                    except,
                    dry_run,
                    allow_exec,
                    allow_external_css,
                },
        } => {
            assert_eq!(input, "bundle.unixnotis");
            assert!(except.is_empty());
            assert!(dry_run);
            assert!(!allow_exec);
            assert!(!allow_external_css);
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_preset_import_external_css_expert_override() {
    let args = Args::try_parse_from([
        "noticenterctl",
        "preset",
        "import",
        "bundle.unixnotis",
        "--allow-external-css",
    ])
    .expect("parse external CSS override");

    assert!(matches!(
        args.command,
        Command::Preset {
            command: PresetCommand::Import {
                allow_external_css: true,
                ..
            }
        }
    ));
}

#[test]
fn parses_preset_inspect() {
    let args = Args::try_parse_from(["noticenterctl", "preset", "inspect", "bundle.unixnotis"])
        .expect("parse args");
    match args.command {
        Command::Preset {
            command: PresetCommand::Inspect { input },
        } => {
            assert_eq!(input, "bundle.unixnotis");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_preset_reset_config_confirmation_flag() {
    let args = Args::try_parse_from(["noticenterctl", "preset", "reset-config", "--yes"])
        .expect("parse reset-config");
    assert!(matches!(
        args.command,
        Command::Preset {
            command: PresetCommand::ResetConfig { yes: true }
        }
    ));
}

#[test]
fn parses_doctor_output_and_service_manager_options() {
    let args = Args::try_parse_from([
        "noticenterctl",
        "doctor",
        "--json",
        "--verbose",
        "--service-manager",
        "dinit",
    ])
    .expect("parse doctor args");

    assert!(matches!(
        args.command,
        Command::Doctor {
            command: None,
            json: true,
            verbose: true,
            service_manager: DoctorServiceManagerArg::Dinit,
            config: None,
        }
    ));
}

#[test]
fn parses_doctor_repair_session_service_manager_without_shell_payloads() {
    let args = Args::try_parse_from([
        "noticenterctl",
        "doctor",
        "repair-session",
        "--service-manager",
        "runit",
    ])
    .expect("parse session environment command");

    assert!(matches!(
        args.command,
        Command::Doctor {
            command: Some(DoctorCommand::RepairSession),
            service_manager: DoctorServiceManagerArg::Runit,
            json: false,
            verbose: false,
            config: None,
        }
    ));
}

#[test]
fn doctor_repair_session_rejects_report_options_and_manual_service_mode() {
    for arguments in [
        vec!["noticenterctl", "doctor", "--json", "repair-session"],
        vec!["noticenterctl", "doctor", "--verbose", "repair-session"],
        vec![
            "noticenterctl",
            "doctor",
            "--config",
            "config.toml",
            "repair-session",
        ],
        vec![
            "noticenterctl",
            "doctor",
            "repair-session",
            "--service-manager",
            "manual",
        ],
    ] {
        let command = Args::try_parse_from(arguments.clone())
            .expect("syntax should parse before semantic validation")
            .command;
        assert!(
            command.validate().is_err(),
            "invalid repair options unexpectedly validated: {arguments:?}"
        );
    }
}

#[test]
fn doctor_and_css_check_accept_explicit_config_paths() {
    let doctor = Args::try_parse_from([
        "noticenterctl",
        "doctor",
        "--config",
        "/tmp/doctor-config.toml",
    ])
    .expect("parse doctor config path");
    assert!(matches!(
        doctor.command,
        Command::Doctor {
            config: Some(path),
            ..
        } if path == std::path::Path::new("/tmp/doctor-config.toml")
    ));

    let css = Args::try_parse_from([
        "noticenterctl",
        "css-check",
        "--config",
        "/tmp/css-config.toml",
    ])
    .expect("parse CSS config path");
    assert!(matches!(
        css.command,
        Command::CssCheck { config: Some(path) }
            if path == std::path::Path::new("/tmp/css-config.toml")
    ));
}

#[test]
fn debug_level_arg_into_panel_level() {
    // Validates CLI debug levels map to the matching control plane enum
    let table = [
        (DebugLevelArg::Critical, PanelDebugLevel::Critical),
        (DebugLevelArg::Warn, PanelDebugLevel::Warn),
        (DebugLevelArg::Info, PanelDebugLevel::Info),
        (DebugLevelArg::Verbose, PanelDebugLevel::Verbose),
    ];
    for (arg, expected) in table {
        let mapped: PanelDebugLevel = arg.into();
        assert_eq!(mapped, expected);
    }
}

#[test]
fn inhibit_scope_arg_maps_to_control_bitmasks() {
    assert_eq!(InhibitScopeArg::All.as_scope(), INHIBIT_SCOPE_ALL);
    assert_eq!(InhibitScopeArg::Popups.as_scope(), INHIBIT_SCOPE_POPUPS);
}
