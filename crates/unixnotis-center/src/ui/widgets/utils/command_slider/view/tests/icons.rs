use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{resolve_slider_icon_name, resolve_symbolic_alias, slider_icon_hints};

#[test]
fn empty_slider_icon_uses_stable_fallback() {
    assert_eq!(
        resolve_slider_icon_name("Volume", "  "),
        "applications-system-symbolic"
    );
}

#[test]
fn slider_icon_hints_recognize_each_label_and_icon_keyword() {
    assert_eq!(slider_icon_hints("Brightness", "custom"), (true, false));
    assert_eq!(
        slider_icon_hints("Custom", "screen-brightness"),
        (true, false)
    );
    assert_eq!(slider_icon_hints("Custom", "video-display"), (true, false));
    assert_eq!(slider_icon_hints("Volume", "custom"), (false, true));
    assert_eq!(slider_icon_hints("Custom", "volume-high"), (false, true));
    assert_eq!(slider_icon_hints("Custom", "audio-output"), (false, true));
    assert_eq!(slider_icon_hints("Custom", "other"), (false, false));
}

#[gtk::test]
fn symbolic_alias_resolution_follows_icons_available_in_theme() {
    let fixture = TestIconTheme::new();

    assert_eq!(
        resolve_symbolic_alias("plain-only-symbolic", &fixture.theme),
        Some("plain-only".to_string())
    );
    assert_eq!(
        resolve_symbolic_alias("symbolic-only", &fixture.theme),
        Some("symbolic-only-symbolic".to_string())
    );
    assert_eq!(resolve_symbolic_alias("missing-icon", &fixture.theme), None);
}

struct TestIconTheme {
    theme: gtk::IconTheme,
    root: PathBuf,
}

impl TestIconTheme {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        // Each test process gets an isolated theme directory under the system temp root
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "unixnotis-slider-icon-theme-{}-{id}",
            std::process::id()
        ));
        let actions = root.join("UnixNotisTest/scalable/actions");
        std::fs::create_dir_all(&actions).expect("test icon directory should be created");
        std::fs::write(
            root.join("UnixNotisTest/index.theme"),
            "[Icon Theme]\nName=UnixNotisTest\nDirectories=scalable/actions\n\n[scalable/actions]\nSize=16\nType=Scalable\nContext=Actions\n",
        )
        .expect("test theme index should be written");
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"16\" height=\"16\"><path d=\"M0 0h16v16H0z\"/></svg>";
        std::fs::write(actions.join("plain-only.svg"), svg)
            .expect("plain test icon should be written");
        std::fs::write(actions.join("symbolic-only-symbolic.svg"), svg)
            .expect("symbolic test icon should be written");

        let theme = gtk::IconTheme::new();
        theme.set_search_path(&[root.as_path()]);
        theme.set_theme_name(Some("UnixNotisTest"));
        assert!(theme.has_icon("plain-only"));
        assert!(theme.has_icon("symbolic-only-symbolic"));

        Self { theme, root }
    }
}

impl Drop for TestIconTheme {
    fn drop(&mut self) {
        // Cleanup is best effort because test assertions should retain the original failure
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
