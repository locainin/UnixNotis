use super::*;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use unixnotis_core::{Config, ThemePaths};
use unixnotis_ui::css::CssManager;

use crate::ui::state::UiState;

#[test]
fn negative_icon_cache_expires_at_the_ttl_boundary() {
    let now = Instant::now();

    let fresh = now
        .checked_sub(Duration::from_secs(14))
        .expect("fresh timestamp should remain representable");
    let expired = now
        .checked_sub(NEGATIVE_ICON_CACHE_TTL)
        .expect("expired timestamp should remain representable");

    assert!(negative_cache_is_fresh(fresh, now));
    assert!(!negative_cache_is_fresh(expired, now));
}

#[test]
fn negative_icon_cache_handles_future_timestamp_without_panicking() {
    let now = Instant::now();

    assert!(negative_cache_is_fresh(now + Duration::from_secs(1), now));
}

#[gtk::test]
fn expired_negative_cache_replaces_its_old_order_marker_once() {
    let mut state = popup_state("org.unixnotis.PopupExpiredIconCache");
    let notification = icon_notification("dialog-information");
    let cache_key = icon_cache_key(&notification);

    state.icon_cache.insert(
        cache_key.clone(),
        IconCacheEntry {
            resolved: None,
            cached_at: Instant::now()
                .checked_sub(NEGATIVE_ICON_CACHE_TTL)
                .expect("test timestamp should remain representable"),
        },
    );
    state.icon_cache_order.push_back(cache_key.clone());

    assert!(state.build_app_icon_widget(&notification, 20).is_some());
    assert_eq!(
        state
            .icon_cache_order
            .iter()
            .filter(|key| *key == &cache_key)
            .count(),
        1
    );
}

#[gtk::test]
fn icon_cache_evicts_only_after_the_configured_limit_is_exceeded() {
    let mut state = popup_state("org.unixnotis.PopupIconCacheLimit");

    for index in 0..ICON_CACHE_MAX_ENTRIES {
        state.cache_icon(
            test_cache_key(&format!("icon-{index}")),
            Some("folder".to_string()),
        );
    }
    assert_eq!(state.icon_cache.len(), ICON_CACHE_MAX_ENTRIES);
    assert!(state.icon_cache.contains_key(&test_cache_key("icon-0")));

    state.cache_icon(
        test_cache_key(&format!("icon-{ICON_CACHE_MAX_ENTRIES}")),
        Some("folder".to_string()),
    );
    assert_eq!(state.icon_cache.len(), ICON_CACHE_MAX_ENTRIES);
    assert!(!state.icon_cache.contains_key(&test_cache_key("icon-0")));
}

#[gtk::test]
fn source_invalidation_discards_successful_resolved_icon_names() {
    let mut state = popup_state("org.unixnotis.PopupIconSourceCacheClear");
    let notification = icon_notification("old-icon");
    let cache_key = icon_cache_key(&notification);

    state.cache_icon(cache_key.clone(), Some("old-icon".to_string()));
    state.icon_cache_order.push_back(cache_key.clone());
    state.icon_sources_dirty.set(true);

    state.invalidate_icon_sources();

    assert!(!state.icon_cache.contains_key(&cache_key));
    assert!(state.icon_cache_order.is_empty());
}

#[test]
fn icon_resolution_key_includes_all_candidate_inputs() {
    let mut first = icon_notification("badge");
    first.attribution.desktop_id = "org.example.First.desktop".to_string();
    first.image.claimed_theme_icon = "first-theme".to_string();
    let mut second = first.clone();
    second.attribution.desktop_id = "org.example.Second.desktop".to_string();
    second.image.claimed_theme_icon = "second-theme".to_string();
    let mut third = first.clone();
    third.image.claimed_desktop_id = "org.example.Third.desktop".to_string();
    let mut fourth = first.clone();
    fourth.attribution.status = unixnotis_core::AttributionStatus::Recognized;

    assert_ne!(icon_cache_key(&first), icon_cache_key(&second));
    assert_ne!(icon_cache_key(&first), icon_cache_key(&third));
    assert_ne!(icon_cache_key(&first), icon_cache_key(&fourth));
}

#[gtk::test]
fn file_icon_rows_keep_the_requested_size_and_cache_small_decodes() {
    let path = test_image_path("spawn-file-icon");
    let image = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_pixel(2, 2, Rgba([1, 2, 3, 255]));
    image
        .save_with_format(&path, ImageFormat::Png)
        .expect("save icon fixture");

    let texture_cache = Rc::new(RefCell::new(TextureCache::new_for_popups()));
    let mut theme_cache = ThemeIconCache::new_for_popups();
    let widget = resolve_icon_widget(
        &mut theme_cache,
        &texture_cache,
        path.to_str().expect("fixture path is utf8"),
        20,
    )
    .expect("regular file icon should create a widget");
    assert_eq!(widget.pixel_size(), 20);

    for _ in 0..100 {
        while gtk::glib::MainContext::default().pending() {
            gtk::glib::MainContext::default().iteration(false);
        }
        if texture_cache.borrow_mut().get(&path, 20).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(texture_cache.borrow_mut().get(&path, 20).is_some());
    let _ = fs::remove_file(path);
}

fn popup_state(application_id: &str) -> UiState {
    let app = gtk::Application::builder()
        .application_id(application_id)
        .flags(gtk::gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gtk::gio::Cancellable>)
        .expect("register icon state test application");

    let config = Config::default();
    let root = std::env::temp_dir().join("unixnotis-popup-icon-state");
    let (command_tx, _command_rx) = tokio::sync::mpsc::channel(4);
    let theme_paths = ThemePaths {
        base_dir: root.clone(),
        base_css: root.join("base.css"),
        popup_css: root.join("popup.css"),
        panel_css: root.join("panel.css"),
        widgets_css: root.join("widgets.css"),
        media_css: root.join("media.css"),
    };
    let css = CssManager::new_popup(theme_paths, config.theme.clone());

    UiState::new(&app, config, root.join("config.toml"), command_tx, css)
}

fn icon_notification(icon_name: &str) -> unixnotis_core::NotificationView {
    unixnotis_core::NotificationView {
        id: 1,
        generation: 1,
        app_name: "Icon test".to_string(),
        attribution: unixnotis_core::NotificationAttribution {
            badge_icon: icon_name.to_string(),
            ..unixnotis_core::NotificationAttribution::default()
        },
        summary: "Icon test".to_string(),
        body: String::new(),
        actions: Vec::new(),
        inline_reply: unixnotis_core::InlineReply::default(),
        inline_reply_policy: unixnotis_core::InlineReplyPolicy::Deny,
        urgency: 1,
        category: String::new(),
        is_transient: false,
        received_at_unix_seconds: 0,
        image: unixnotis_core::NotificationImage::default(),
        popup_decision: unixnotis_core::PopupDecisionRecord::default(),
        popup_hide_after_ms: 0,
    }
}

fn icon_cache_key(notification: &unixnotis_core::NotificationView) -> IconResolutionKey {
    IconResolutionKey {
        app_name: notification.app_name.clone(),
        badge_icon: notification.attribution.badge_icon.clone(),
        desktop_id: notification.attribution.desktop_id.clone(),
        claimed_theme_icon: notification.image.claimed_theme_icon.clone(),
        claimed_desktop_id: notification.image.claimed_desktop_id.clone(),
        claimed_candidates_first: notification.attribution.status
            == unixnotis_core::AttributionStatus::Unresolved,
    }
}

fn test_cache_key(name: &str) -> IconResolutionKey {
    IconResolutionKey {
        app_name: name.to_string(),
        badge_icon: String::new(),
        desktop_id: String::new(),
        claimed_theme_icon: String::new(),
        claimed_desktop_id: String::new(),
        claimed_candidates_first: true,
    }
}

fn test_image_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be valid")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "unixnotis-popups-{name}-{}-{nonce}.png",
        std::process::id()
    ))
}
