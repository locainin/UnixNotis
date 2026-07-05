use clap::Parser;

use super::super::{Args, Command, DebugLevelArg, DndState, InhibitScopeArg, PresetCommand};

#[test]
fn parses_open_panel_debug_default() {
    // Ensures clap default_missing_value maps --debug to the Info level
    let args =
        Args::try_parse_from(["noticenterctl", "open-panel", "--debug"]).expect("parse args");
    match args.command {
        Command::OpenPanel { debug } => {
            assert!(matches!(debug, Some(DebugLevelArg::Info)));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_open_panel_debug_value() {
    // Verifies explicit debug values map to the requested verbosity
    let args = Args::try_parse_from(["noticenterctl", "open-panel", "--debug", "verbose"])
        .expect("parse args");
    match args.command {
        Command::OpenPanel { debug } => {
            assert!(matches!(debug, Some(DebugLevelArg::Verbose)));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_dnd_toggle() {
    // Confirms the value enum accepts the toggle state for DND commands
    let args = Args::try_parse_from(["noticenterctl", "dnd", "toggle"]).expect("parse args");
    match args.command {
        Command::Dnd { state } => {
            assert!(matches!(state, DndState::Toggle));
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn parses_explicit_clear_variants() {
    for (name, expected) in [
        ("clear", "clear"),
        ("clear-all", "clear-all"),
        ("clear-active", "clear-active"),
        ("clear-history", "clear-history"),
    ] {
        let args = Args::try_parse_from(["noticenterctl", name]).expect("parse args");
        match (args.command, expected) {
            (Command::Clear, "clear")
            | (Command::ClearAll, "clear-all")
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
                },
        } => {
            assert_eq!(input, "bundle.unixnotis");
            assert!(except.is_empty());
            assert!(dry_run);
            assert!(!allow_exec);
        }
        other => panic!("unexpected command: {other:?}"),
    }
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
