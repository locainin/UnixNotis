use std::num::NonZeroU32;

use crate::store::{StableProcessIdentity, SuppressedNotification};

use super::{PostReplyKey, PostReplyLifecycle, RetainError, MAX_PENDING_SUPPRESSED_CLOSES};

fn suppressed(id: u32, generation: u64) -> SuppressedNotification {
    SuppressedNotification {
        id,
        generation,
        owner: Some(StableProcessIdentity {
            pid: id,
            start_time: generation,
        }),
    }
}

fn request(sender: &str, serial: u32) -> PostReplyKey {
    PostReplyKey {
        sender: Some(sender.to_string()),
        serial: NonZeroU32::new(serial).expect("non-zero request serial"),
    }
}

#[tokio::test]
async fn retained_lifecycle_is_removed_only_by_its_request_serial() {
    let lifecycle = PostReplyLifecycle::default();
    let first_request = request(":1.11", 11);
    let second_request = request(":1.11", 12);
    let first = suppressed(21, 31);
    let second = suppressed(22, 32);

    lifecycle
        .retain(first_request.clone(), first)
        .await
        .expect("first request serial should be vacant");
    lifecycle
        .retain(second_request.clone(), second)
        .await
        .expect("second request serial should be vacant");

    assert_eq!(lifecycle.take(&second_request).await, Some(second));
    assert_eq!(lifecycle.take(&first_request).await, Some(first));
    assert_eq!(lifecycle.take(&first_request).await, None);
}

#[tokio::test]
async fn duplicate_in_flight_serial_keeps_the_original_lifecycle() {
    let lifecycle = PostReplyLifecycle::default();
    let request = request(":1.41", 41);
    let original = suppressed(51, 61);
    let duplicate = suppressed(52, 62);

    lifecycle
        .retain(request.clone(), original)
        .await
        .expect("first request serial should be vacant");
    assert_eq!(
        lifecycle.retain(request.clone(), duplicate).await,
        Err(RetainError::DuplicateSerial)
    );
    assert_eq!(lifecycle.take(&request).await, Some(original));
}

#[tokio::test]
async fn equal_serials_from_different_senders_keep_independent_lifecycles() {
    let lifecycle = PostReplyLifecycle::default();
    let first_request = request(":1.51", 1);
    let second_request = request(":1.52", 1);
    let first = suppressed(71, 81);
    let second = suppressed(72, 82);

    lifecycle
        .retain(first_request.clone(), first)
        .await
        .expect("first sender should have an independent serial space");
    lifecycle
        .retain(second_request.clone(), second)
        .await
        .expect("second sender should have an independent serial space");

    assert_eq!(lifecycle.take(&first_request).await, Some(first));
    assert_eq!(lifecycle.take(&second_request).await, Some(second));
}

#[tokio::test]
async fn pending_lifecycle_capacity_is_hard_bounded_and_reusable() {
    let lifecycle = PostReplyLifecycle::default();

    for serial in 1..=MAX_PENDING_SUPPRESSED_CLOSES {
        let serial = u32::try_from(serial).expect("test capacity fits u32");
        let request = request(":1.capacity", serial);
        lifecycle
            .retain(request, suppressed(serial, u64::from(serial)))
            .await
            .expect("exact queue capacity should be accepted");
    }

    let overflow_serial =
        u32::try_from(MAX_PENDING_SUPPRESSED_CLOSES + 1).expect("test overflow serial fits u32");
    let overflow_request = request(":1.capacity", overflow_serial);
    assert_eq!(
        lifecycle
            .retain(overflow_request.clone(), suppressed(overflow_serial, 1))
            .await,
        Err(RetainError::CapacityExceeded)
    );

    let released_request = request(":1.capacity", 1);
    assert!(lifecycle.take(&released_request).await.is_some());
    lifecycle
        .retain(overflow_request, suppressed(overflow_serial, 2))
        .await
        .expect("released capacity should admit the next lifecycle");
}
