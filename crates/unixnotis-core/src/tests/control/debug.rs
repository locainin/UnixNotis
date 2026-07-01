use super::{should_archive_closed_notification, CloseReason, PanelDebugLevel};

#[test]
fn user_dismiss_never_archives() {
    assert!(!should_archive_closed_notification(
        CloseReason::DismissedByUser,
        false,
        true
    ));
    assert!(!should_archive_closed_notification(
        CloseReason::DismissedByUser,
        true,
        true
    ));
}

#[test]
fn transient_archive_follows_config() {
    assert!(!should_archive_closed_notification(
        CloseReason::Expired,
        true,
        false
    ));
    assert!(should_archive_closed_notification(
        CloseReason::Expired,
        true,
        true
    ));
}

#[test]
fn non_transient_close_still_archives() {
    assert!(should_archive_closed_notification(
        CloseReason::Expired,
        false,
        false
    ));
    assert!(should_archive_closed_notification(
        CloseReason::ClosedByCall,
        false,
        true
    ));
}

#[test]
fn panel_debug_level_off_allows_no_diagnostics() {
    // Off is a hard gate so accidental ordering changes cannot enable logging
    assert!(!PanelDebugLevel::Off.allows(PanelDebugLevel::Off));
    assert!(!PanelDebugLevel::Off.allows(PanelDebugLevel::Critical));
    assert!(!PanelDebugLevel::Off.allows(PanelDebugLevel::Verbose));
}

#[test]
fn panel_debug_level_allows_requested_level_and_lower_severity() {
    // Higher verbosity accepts less chatty levels because the enum is ordered by detail
    assert!(PanelDebugLevel::Critical.allows(PanelDebugLevel::Critical));
    assert!(PanelDebugLevel::Warn.allows(PanelDebugLevel::Critical));
    assert!(PanelDebugLevel::Info.allows(PanelDebugLevel::Warn));
    assert!(PanelDebugLevel::Verbose.allows(PanelDebugLevel::Info));
    assert!(PanelDebugLevel::Verbose.allows(PanelDebugLevel::Verbose));
}

#[test]
fn panel_debug_level_rejects_more_verbose_requests() {
    // Warn must not permit Info or Verbose logs because those are noisier than requested
    assert!(!PanelDebugLevel::Critical.allows(PanelDebugLevel::Warn));
    assert!(!PanelDebugLevel::Warn.allows(PanelDebugLevel::Info));
    assert!(!PanelDebugLevel::Info.allows(PanelDebugLevel::Verbose));
}
