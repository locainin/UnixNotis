use super::{Backoff, BACKOFF_MAX_MS};

#[test]
fn reconnect_delay_never_exceeds_shared_maximum() {
    let mut backoff = Backoff::new(BACKOFF_MAX_MS, BACKOFF_MAX_MS);

    for _ in 0..32 {
        assert!(backoff.next_sleep().as_millis() <= u128::from(BACKOFF_MAX_MS));
    }
}
