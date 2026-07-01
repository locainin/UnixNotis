use std::path::{Path, PathBuf};

use gtk::gdk;
use gtk::prelude::FileExt;

use super::super::{is_missing_icon, resolve_icon_image, resolve_icon_paintable};
use super::support::texture_test_lock;

fn available_theme_icon() -> Option<&'static str> {
    // Theme lookup needs GTK initialized before a default display is available
    gtk::init().ok()?;
    let display = gdk::Display::default()?;
    let theme = gtk::IconTheme::for_display(&display);

    // Common freedesktop names keep the test portable across normal desktop themes
    ["folder", "dialog-information", "image-x-generic"]
        .into_iter()
        .find(|name| theme.has_icon(name))
}

#[test]
fn is_missing_icon_detects_theme_placeholder_stems_only() {
    assert!(is_missing_icon(Path::new("/theme/image-missing.svg")));
    assert!(is_missing_icon(Path::new(
        "/theme/image-missing-symbolic.png"
    )));
    assert!(!is_missing_icon(Path::new("/theme/message-new.svg")));
    assert!(!is_missing_icon(Path::new(
        "/theme/image-missing-folder/icon.svg"
    )));
}

#[test]
fn resolve_icon_helpers_reject_empty_icon_names() {
    assert!(resolve_icon_paintable("", 24).is_none());
    assert!(resolve_icon_image("", 24).is_none());
}

#[test]
fn resolve_icon_image_uses_theme_icon_and_sets_requested_size() {
    let _guard = texture_test_lock();
    let Some(icon_name) = available_theme_icon() else {
        // Headless test runs do not have a GTK display or icon theme to query
        return;
    };

    let paintable = resolve_icon_paintable(icon_name, 24).expect("theme icon paintable");
    assert!(!is_missing_icon(
        &paintable
            .file()
            .and_then(|file| file.path())
            .unwrap_or_else(|| PathBuf::from(icon_name))
    ));

    let image = resolve_icon_image(icon_name, 24).expect("theme icon image");

    assert_eq!(image.pixel_size(), 24);
}
