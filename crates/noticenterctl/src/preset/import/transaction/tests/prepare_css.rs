use super::*;

#[test]
fn import_rejects_outside_css_asset_refs_in_noninteractive_runs() {
    // Shared presets should not silently import CSS that reaches outside the config root for assets
    let export_root = TempDirGuard::new("external-css-asset-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write(
        "base.css",
        ".panel { background-image: url(\"../outside.png\"); }\n",
    );
    let bundle_path = export_root.path.join("demo.unixnotis");
    write_collected_bundle(
        &export_root,
        &bundle_path,
        "2026-04-11T00:00:00Z",
        &[("config.toml", "config.toml"), ("base.css", "base.css")],
    );

    let import_root = TempDirGuard::new("external-css-asset-import");
    // The injected rejector makes this test stable even when cargo test is attached to a tty
    let error = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        false,
        false,
        |_refs| {
            Err(anyhow::anyhow!(
                "preset import found CSS asset references that leave the UnixNotis config directory or use remote URLs"
            ))
        },
        |_exec_content, _allow_exec| Ok(()),
    )
    .expect_err("reject outside css asset refs without confirmation");

    assert!(error.to_string().contains(
        "CSS asset references that leave the UnixNotis config directory or use remote URLs"
    ));
    assert!(!import_root.path.join("config.toml").exists());
}

#[test]
fn import_skips_css_asset_warning_for_excluded_stylesheet() {
    let export_root = TempDirGuard::new("excluded-css-warning-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write("base.css", ".panel { color: red; }\n");
    export_root.write(
        "assets.css",
        ".panel { background-image: url(\"../outside.png\"); }\n",
    );
    let bundle_path = export_root.path.join("demo.unixnotis");
    write_collected_bundle(
        &export_root,
        &bundle_path,
        "2026-04-16T00:00:00Z",
        &[
            ("config.toml", "config.toml"),
            ("base.css", "base.css"),
            ("assets.css", "assets.css"),
        ],
    );

    let import_root = TempDirGuard::new("excluded-css-warning-import");
    let summary = import_preset_into_with_confirm(
        &import_root.path,
        &bundle_path,
        &["assets.css".to_string()],
        false,
        false,
        |refs: &[crate::preset::css_asset_refs::ExternalCssAssetRef]| {
            assert!(refs.is_empty());
            Ok(())
        },
        |_exec_content, _allow_exec| Ok(()),
    )
    .expect("ignore excluded stylesheet warning");

    assert_eq!(summary.file_count, 2);
    assert!(!import_root.path.join("assets.css").exists());
}

#[test]
fn import_rejects_escaped_external_url_and_import_tokens_before_writing() {
    for (label, css) in [
        (
            "escaped-url",
            r#".panel { background-image: u\72l("file:///dev/zero"); }"#,
        ),
        (
            "six-digit-url",
            r#".panel { background-image: U\000052L("https://example.invalid/a.png"); }"#,
        ),
        ("escaped-import", r#"@im\70ort "file:///dev/zero";"#),
    ] {
        let export_root = TempDirGuard::new(&format!("{label}-export"));
        export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
        export_root.write("base.css", css);
        let bundle_path = export_root.path.join("demo.unixnotis");
        write_collected_bundle(
            &export_root,
            &bundle_path,
            "2026-07-18T00:00:00Z",
            &[("config.toml", "config.toml"), ("base.css", "base.css")],
        );
        let import_root = TempDirGuard::new(&format!("{label}-import"));

        let error = import_preset_into(&import_root.path, &bundle_path, &[], false)
            .expect_err("reject escaped external CSS reference");

        assert!(error.to_string().contains("--allow-external-css"));
        assert!(!import_root.path.join("config.toml").exists());
    }
}

#[test]
fn external_css_override_still_routes_findings_through_confirmation() {
    let export_root = TempDirGuard::new("external-css-override-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write(
        "base.css",
        r#".panel { background-image: u\72l("https://example.invalid/a.png"); }"#,
    );
    let bundle_path = export_root.path.join("demo.unixnotis");
    write_collected_bundle(
        &export_root,
        &bundle_path,
        "2026-07-18T00:00:00Z",
        &[("config.toml", "config.toml"), ("base.css", "base.css")],
    );
    let import_root = TempDirGuard::new("external-css-override-import");

    let summary = import_preset_into_with_policy_and_confirm(
        &import_root.path,
        &bundle_path,
        &[],
        true,
        crate::preset::import::transaction::prepare::ImportTrustPolicy {
            allow_exec: false,
            allow_external_css: true,
        },
        |refs| {
            assert_eq!(refs.len(), 1);
            assert_eq!(refs[0].reason, "remote url");
            Ok(())
        },
        |_exec_content, _allow_exec| Ok(()),
    )
    .expect("allow explicitly approved external CSS");

    assert!(summary.dry_run);
    assert!(!import_root.path.join("config.toml").exists());
}

#[test]
fn import_materializes_data_images_before_publishing_stylesheets() {
    let export_root = TempDirGuard::new("data-css-image-export");
    export_root.write("config.toml", "[theme]\nbase_css = \"base.css\"\n");
    export_root.write(
        "base.css",
        concat!(
            ".panel { background-image: url(\"data:image/png;base64,",
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+",
            "A8AAQUBAScY42YAAAAASUVORK5CYII=\"); }",
        ),
    );
    let bundle_path = export_root.path.join("demo.unixnotis");
    write_collected_bundle(
        &export_root,
        &bundle_path,
        "2026-07-18T00:00:00Z",
        &[("config.toml", "config.toml"), ("base.css", "base.css")],
    );
    let import_root = TempDirGuard::new("data-css-image-import");

    let summary = import_preset_into(&import_root.path, &bundle_path, &[], false)
        .expect("import bounded data image");
    let imported_css =
        fs::read_to_string(import_root.path.join("base.css")).expect("read imported stylesheet");
    let generated_dir = import_root.path.join("assets/.validated-css");
    let generated = fs::read_dir(&generated_dir)
        .expect("read validated CSS asset directory")
        .collect::<std::io::Result<Vec<_>>>()
        .expect("collect validated CSS assets");

    assert_eq!(summary.file_count, 3);
    assert!(!imported_css.contains("data:image"));
    assert!(imported_css.contains("assets/.validated-css/"));
    assert_eq!(generated.len(), 1);
    assert!(fs::read(generated[0].path())
        .expect("read validated CSS image")
        .starts_with(b"\x89PNG"));
}
