use crate::store::DismissOutcome;
use unixnotis_core::NotificationKey;

#[test]
fn dismiss_outcome_reports_any_removed_side() {
    assert!(DismissOutcome {
        removed_active: Some(NotificationKey {
            id: 1,
            generation: 1,
        }),
        removed_history: None,
    }
    .removed_any());
    assert!(DismissOutcome {
        removed_active: None,
        removed_history: Some(NotificationKey {
            id: 2,
            generation: 2,
        }),
    }
    .removed_any());
    assert!(!DismissOutcome {
        removed_active: None,
        removed_history: None,
    }
    .removed_any());
}
