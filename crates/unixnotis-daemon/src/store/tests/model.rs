use crate::store::DismissOutcome;

#[test]
fn dismiss_outcome_reports_any_removed_side() {
    assert!(DismissOutcome {
        removed_active: true,
        removed_history: false,
    }
    .removed_any());
    assert!(DismissOutcome {
        removed_active: false,
        removed_history: true,
    }
    .removed_any());
    assert!(!DismissOutcome {
        removed_active: false,
        removed_history: false,
    }
    .removed_any());
}
