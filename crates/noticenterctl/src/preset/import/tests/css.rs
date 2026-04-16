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
            Err(anyhow!(
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
