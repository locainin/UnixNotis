use super::*;

#[test]
fn import_rejects_command_bearing_bundle_by_default() {
    let export_root = TempDirGuard::new("command-bearing-export");
    export_root.write(
        "config.toml",
        r#"[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/probe.sh"
"#,
    );
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("command-bearing-import");
    let error = import_preset_into(&import_root.path, &bundle_path, &[], false)
        .expect_err("reject executable preset by default");

    assert!(error
        .to_string()
        .contains("rerun interactively to inspect them or use --allow-exec"));
    assert!(!import_root.path.join("config.toml").exists());
}

#[test]
fn import_skips_exec_review_for_excluded_script_payload() {
    let export_root = TempDirGuard::new("excluded-script-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("excluded-script-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &["scripts".to_string()],
        false,
        false,
        |_refs| Ok(()),
        |exec_content, allow_exec| {
            assert!(!allow_exec);
            assert!(exec_content.commands.is_empty());
            assert!(exec_content.files.is_empty());
            Ok(())
        },
    )
    .expect("ignore excluded script payload");

    assert_eq!(summary.file_count, 2);
    assert!(!import_root.path.join("scripts/probe.sh").exists());
}

#[test]
fn import_allows_command_bearing_bundle_when_explicitly_trusted() {
    let export_root = TempDirGuard::new("command-allowed-export");
    export_root.write(
        "config.toml",
        r#"[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/probe.sh"
"#,
    );
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("command-allowed-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        false,
        true,
        |_refs| Ok(()),
        |_exec_content, _allow_exec| Ok(()),
    )
    .expect("allow trusted executable preset");

    assert_eq!(summary.file_count, 3);
    assert!(import_root.path.join("scripts/probe.sh").exists());
}

#[test]
fn import_exec_review_receives_every_included_file_before_confirmation() {
    let export_root = TempDirGuard::new("command-reviewed-export");
    export_root.write(
        "config.toml",
        r#"[theme]
base_css = "base.css"
[[widgets.stats]]
label = "Probe"
cmd = "scripts/probe.sh"
"#,
    );
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("scripts/probe.sh", "#!/bin/sh\necho ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    export_preset_from(&export_root.path, &bundle_path, &[], false).expect("export bundle");

    let import_root = TempDirGuard::new("command-reviewed-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        false,
        false,
        |_refs| Ok(()),
        |exec_content, allow_exec| {
            assert!(!allow_exec);
            assert_eq!(exec_content.commands.len(), 1);
            assert_eq!(exec_content.files.len(), 3);
            assert!(exec_content
                .files
                .iter()
                .any(|file| file.relative_path == Path::new("base.css")));
            Ok(())
        },
    )
    .expect("allow reviewed executable preset");

    assert_eq!(summary.file_count, 3);
    assert!(import_root.path.join("scripts/probe.sh").exists());
}

#[test]
fn import_review_ignores_unknown_command_decoys_and_includes_plain_payload() {
    let export_root = TempDirGuard::new("command-decoy-export");
    let mut config = String::from("[theme]\nbase_css = \"base.css\"\n");
    for index in 0..64 {
        config.push_str(&format!("[aaa{index:02}]\ncmd = \"true\"\n"));
    }
    config.push_str("[[widgets.stats]]\nlabel = \"Probe\"\ncmd = \"sh assets/payload.dat\"\n");
    export_root.write("config.toml", &config);
    export_root.write("base.css", ".probe { color: red; }");
    export_root.write("assets/payload.dat", "printf '%s\\n' ok\n");
    let bundle_path = export_root.path.join("demo.unixnotis");
    write_collected_bundle(
        &export_root,
        &bundle_path,
        "2025-01-01T00:00:00Z",
        &[
            ("config.toml", "config.toml"),
            ("base.css", "base.css"),
            ("assets/payload.dat", "assets/payload.dat"),
        ],
    );

    let import_root = TempDirGuard::new("command-decoy-import");
    import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        false,
        false,
        |_refs| Ok(()),
        |exec_content, allow_exec| {
            assert!(!allow_exec);
            assert_eq!(exec_content.commands.len(), 1);
            assert_eq!(exec_content.commands[0].slot, "widgets.stats[0].cmd");
            assert!(exec_content
                .files
                .iter()
                .any(|file| file.relative_path == Path::new("assets/payload.dat")));
            Ok(())
        },
    )
    .expect("allow complete review model");

    assert!(import_root.path.join("assets/payload.dat").exists());
}
