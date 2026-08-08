use std::collections::HashMap;
use std::num::NonZeroU32;

use tokio::sync::Mutex;
use zbus::message::Header;

use crate::store::SuppressedNotification;

const MAX_PENDING_SUPPRESSED_CLOSES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RetainError {
    CapacityExceeded,
    DuplicateSerial,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct PostReplyKey {
    sender: Option<String>,
    serial: NonZeroU32,
}

impl PostReplyKey {
    pub(super) fn from_header(header: &Header<'_>) -> Self {
        Self {
            // The bus name is transport correlation only and grants no ownership
            sender: header.sender().map(ToString::to_string),
            serial: header.primary().serial_num(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NotifyCompletion {
    pub(super) id: u32,
    pub(super) suppressed: Option<SuppressedNotification>,
}

/// Content-free lifecycle work held until its method reply crosses D-Bus
#[derive(Default)]
pub(super) struct PostReplyLifecycle {
    // Sender and serial together identify one in-flight method reply
    pending: Mutex<HashMap<PostReplyKey, SuppressedNotification>>,
}

impl PostReplyLifecycle {
    pub(super) async fn retain(
        &self,
        request: PostReplyKey,
        suppressed: SuppressedNotification,
    ) -> Result<(), RetainError> {
        let mut pending = self.pending.lock().await;
        // An in-flight request must never identify two returned IDs
        if pending.contains_key(&request) {
            return Err(RetainError::DuplicateSerial);
        }
        // A stalled transport cannot grow deferred lifecycle memory without bound
        if pending.len() >= MAX_PENDING_SUPPRESSED_CLOSES {
            return Err(RetainError::CapacityExceeded);
        }
        pending.insert(request, suppressed);
        Ok(())
    }

    pub(super) async fn take(&self, request: &PostReplyKey) -> Option<SuppressedNotification> {
        self.pending.lock().await.remove(request)
    }
}

#[cfg(test)]
#[path = "tests/reply_lifecycle.rs"]
mod tests;
