//! Notification D-Bus interface implementation

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Semaphore;
use tracing::debug;
use zbus::message::Header;
use zbus::{interface, SignalContext};

use crate::expire::ExpirationScheduler;

use super::capabilities::notification_capabilities;
use super::reply_lifecycle::{PostReplyKey, PostReplyLifecycle, RetainError};
use super::wire_hints::WireHints;
use crate::daemon::notifications::ingress::metrics::{IngressMetrics, RejectedRequest};
use crate::daemon::notifications::ingress::quota::NotificationQuota;
use crate::daemon::DaemonState;

const MAX_CONCURRENT_NOTIFY_HANDLERS: usize = 8;

/// D-Bus server for org.freedesktop.Notifications
pub struct NotificationServer {
    // Shared daemon state for store access, sounds, and signal emission
    pub(super) state: Arc<DaemonState>,
    // Scheduler handles expiration deadlines without blocking D-Bus handlers
    pub(super) scheduler: ExpirationScheduler,
    // Shared token buckets reject sustained sender and process-wide floods
    pub(super) notify_quota: NotificationQuota,
    // Close requests are cheaper but still trigger sender identity and store work
    pub(super) close_quota: NotificationQuota,
    // Expensive sender and payload work has a fixed concurrency ceiling
    notify_slots: Semaphore,
    // Counters expose pressure without retaining attacker-controlled labels
    pub(super) ingress_metrics: IngressMetrics,
    // DropAll lifecycle records wait here until the matching reply is sent
    pub(super) post_reply_lifecycle: PostReplyLifecycle,
}

impl NotificationServer {
    pub fn new(state: Arc<DaemonState>, scheduler: ExpirationScheduler) -> Self {
        // Keep constructor minimal and explicit
        Self {
            state,
            scheduler,
            notify_quota: NotificationQuota::new_notify(),
            close_quota: NotificationQuota::new_close(),
            notify_slots: Semaphore::const_new(MAX_CONCURRENT_NOTIFY_HANDLERS),
            ingress_metrics: IngressMetrics::new(),
            post_reply_lifecycle: PostReplyLifecycle::default(),
        }
    }
}

#[interface(name = "org.freedesktop.Notifications")]
impl NotificationServer {
    pub(super) async fn get_capabilities(&self) -> Vec<String> {
        // Advertise sender sound support only when every promised hint is implemented
        notification_capabilities(self.state.sound.supports_fdo_sound_capability())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the freedesktop notification D-Bus signature fixes this argument list"
    )]
    pub(super) async fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: WireHints,
        #[zbus(header)] header: Header<'_>,
        expire_timeout: i32,
    ) -> zbus::fdo::Result<u32> {
        if !self.notify_quota.admit_global(Instant::now()) {
            let rejected = self
                .ingress_metrics
                .record_rejection(RejectedRequest::NotifyQuota);
            debug!(rejected, "notification request rejected by ingress quota");
            return Err(zbus::fdo::Error::LimitsExceeded(
                "notification ingress quota exceeded".to_string(),
            ));
        }
        let _slot = self.notify_slots.try_acquire().map_err(|_error| {
            let rejected = self
                .ingress_metrics
                .record_rejection(RejectedRequest::NotifyConcurrency);
            debug!(
                rejected,
                "notification request rejected by concurrency limit"
            );
            zbus::fdo::Error::LimitsExceeded(
                "too many concurrent notification requests".to_string(),
            )
        })?;
        let _activity = self.ingress_metrics.enter_handler();
        // The interface adapter forwards the authenticated header with the exact wire payload
        let completion = self
            .ingest_notify_deferred(
                app_name,
                replaces_id,
                app_icon,
                summary,
                body,
                actions,
                hints,
                &header,
                expire_timeout,
            )
            .await?;
        if let Some(suppressed) = completion.suppressed {
            let request = PostReplyKey::from_header(&header);
            self.post_reply_lifecycle
                .retain(request, suppressed)
                .await
                .map_err(|error| match error {
                    RetainError::CapacityExceeded => zbus::fdo::Error::LimitsExceeded(
                        "notification lifecycle queue is full".to_string(),
                    ),
                    RetainError::DuplicateSerial => zbus::fdo::Error::Failed(
                        "notification lifecycle request collision".to_string(),
                    ),
                })?;
        }
        Ok(completion.id)
    }

    pub(super) async fn close_notification(
        &self,
        id: u32,
        #[zbus(header)] header: Header<'_>,
    ) -> zbus::fdo::Result<()> {
        if !self.close_quota.admit_global(Instant::now()) {
            let rejected = self
                .ingress_metrics
                .record_rejection(RejectedRequest::CloseQuota);
            debug!(rejected, "close request rejected by ingress quota");
            return Err(zbus::fdo::Error::LimitsExceeded(
                "notification close quota exceeded".to_string(),
            ));
        }
        let _activity = self.ingress_metrics.enter_handler();
        // Ownership checks remain in the shared close path used by all D-Bus callers
        self.close_notification_if_owned(id, &header).await
    }

    pub(super) async fn get_server_information(&self) -> (String, String, String, String) {
        // Keep server information stable for freedesktop client compatibility
        (
            "UnixNotis".to_string(),
            "UnixNotis".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
            "1.2".to_string(),
        )
    }

    #[zbus(signal)]
    // Signal declarations define the freedesktop wire contract and are emitted elsewhere
    pub(crate) async fn notification_closed(
        ctx: &SignalContext<'_>,
        id: u32,
        reason: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    // Action keys are passed through unchanged so clients can match their registered action
    pub(crate) async fn action_invoked(
        ctx: &SignalContext<'_>,
        id: u32,
        action_key: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    // KDE-compatible senders receive the entered text through this extension signal
    pub(crate) async fn notification_replied(
        ctx: &SignalContext<'_>,
        id: u32,
        reply_text: &str,
    ) -> zbus::Result<()>;
}
