use std::time::{Duration, Instant};

use super::{seed_retry_deadline, SEED_RETRY_BUDGET_SECS};

#[test]
fn seed_retry_deadline_adds_the_fixed_retry_budget() {
    let now = Instant::now();

    assert_eq!(
        seed_retry_deadline(now),
        now + Duration::from_secs(SEED_RETRY_BUDGET_SECS)
    );
}
