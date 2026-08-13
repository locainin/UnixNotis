//! Notification add, update, close, eviction, and bulk-clear fanout

use futures_util::stream::{self, StreamExt};
use tracing::warn;
use unixnotis_core::{CloseReason, NotificationKey};

use crate::daemon::{ControlServer, DaemonState, NotificationServer, NotificationSignalMode};

use super::publisher::{record_first_error, DaemonEventPublisher};

const CLEAR_ALL_CONCURRENCY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ClearAllSignalPlan {
    pub(super) publish_close_signals: bool,
    pub(super) publish_snapshot_invalidated: bool,
    pub(super) publish_state_changed: bool,
}

pub(super) const fn clear_all_signal_plan(keys: &[NotificationKey]) -> ClearAllSignalPlan {
    ClearAllSignalPlan {
        publish_close_signals: !keys.is_empty(),
        // Empty clears remain a recovery path for stale materialized client views
        publish_snapshot_invalidated: true,
        publish_state_changed: true,
    }
}

impl DaemonState {
    pub(crate) async fn publish_notification_closed(
        &self,
        key: NotificationKey,
        reason: CloseReason,
    ) -> zbus::Result<()> {
        let mut first_error = self
            .events
            .notification_closed(key, reason, true)
            .await
            .err();
        if let Err(error) = self.publish_state_changed().await {
            record_first_error(&mut first_error, error);
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(in crate::daemon) async fn publish_notification_dismissed(
        &self,
        key: NotificationKey,
        removed_active: bool,
    ) -> zbus::Result<()> {
        let mut first_error = self
            .events
            .notification_closed(key, CloseReason::DismissedByUser, removed_active)
            .await
            .err();
        if let Err(error) = self.publish_state_changed().await {
            record_first_error(&mut first_error, error);
        }
        first_error.map_or(Ok(()), Err)
    }

    pub(in crate::daemon) async fn publish_notification_change(
        &self,
        mode: NotificationSignalMode,
        key: NotificationKey,
        replaced: bool,
    ) -> zbus::Result<()> {
        self.events.notification_change(mode, key, replaced).await
    }

    pub(in crate::daemon) async fn publish_evicted_notifications(
        &self,
        keys: &[NotificationKey],
    ) -> zbus::Result<()> {
        self.events.evicted_notifications(keys).await
    }

    pub(in crate::daemon) async fn publish_notifications_cleared(
        &self,
        keys: Vec<NotificationKey>,
    ) {
        let plan = clear_all_signal_plan(&keys);
        if plan.publish_close_signals {
            if let Err(error) = self.events.cleared_notifications(keys).await {
                warn!(
                    ?error,
                    "notification clear committed but close fanout failed"
                );
            }
        }
        if plan.publish_snapshot_invalidated {
            if let Err(error) = self.publish_snapshot_invalidated().await {
                warn!(
                    ?error,
                    "notification clear committed but snapshot invalidation failed"
                );
            }
        }
        if plan.publish_state_changed {
            if let Err(error) = self.publish_state_changed().await {
                warn!(
                    ?error,
                    "notification clear committed but state fanout failed"
                );
            }
        }
    }
}

impl DaemonEventPublisher {
    async fn notification_closed(
        &self,
        key: NotificationKey,
        reason: CloseReason,
        publish_freedesktop: bool,
    ) -> zbus::Result<()> {
        let mut first_error = None;
        if publish_freedesktop {
            match self.notification_context() {
                Ok(context) => {
                    if let Err(error) =
                        NotificationServer::notification_closed(&context, key.id, reason as u32)
                            .await
                    {
                        record_first_error(&mut first_error, error);
                    }
                }
                Err(error) => record_first_error(&mut first_error, error),
            }
        }
        match self.control_context() {
            Ok(context) => {
                if let Err(error) =
                    ControlServer::notification_closed(&context, key.id, key.generation, reason)
                        .await
                {
                    record_first_error(&mut first_error, error);
                }
            }
            Err(error) => record_first_error(&mut first_error, error),
        }
        first_error.map_or(Ok(()), Err)
    }

    async fn notification_change(
        &self,
        mode: NotificationSignalMode,
        key: NotificationKey,
        replaced: bool,
    ) -> zbus::Result<()> {
        match mode {
            NotificationSignalMode::Direct => {
                let context = self.control_context()?;
                if replaced {
                    ControlServer::notification_updated(&context, key.id, key.generation).await
                } else {
                    ControlServer::notification_added(&context, key.id, key.generation).await
                }
            }
            NotificationSignalMode::SnapshotOnly => self.snapshot_invalidated().await,
        }
    }

    async fn evicted_notifications(&self, keys: &[NotificationKey]) -> zbus::Result<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let notification_context = self.notification_context()?;
        let control_context = self.control_context()?;
        let mut first_error = None;
        for &key in keys {
            if let Err(error) = NotificationServer::notification_closed(
                &notification_context,
                key.id,
                CloseReason::Undefined as u32,
            )
            .await
            {
                record_first_error(&mut first_error, error);
            }
            if let Err(error) = ControlServer::notification_closed(
                &control_context,
                key.id,
                key.generation,
                CloseReason::Undefined,
            )
            .await
            {
                record_first_error(&mut first_error, error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    async fn cleared_notifications(&self, keys: Vec<NotificationKey>) -> zbus::Result<()> {
        let notification_context = self.notification_context()?;
        let control_context = self.control_context()?;
        let first_error = std::sync::Mutex::new(None);

        // Contexts are reused and concurrency remains bounded for large configured stores
        stream::iter(keys)
            .for_each_concurrent(CLEAR_ALL_CONCURRENCY, |key| {
                let notification_context = notification_context.clone();
                let control_context = control_context.clone();
                let first_error = &first_error;
                async move {
                    if let Err(error) = NotificationServer::notification_closed(
                        &notification_context,
                        key.id,
                        CloseReason::DismissedByUser as u32,
                    )
                    .await
                    {
                        let mut first = first_error
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        record_first_error(&mut first, error);
                    }
                    if let Err(error) = ControlServer::notification_closed(
                        &control_context,
                        key.id,
                        key.generation,
                        CloseReason::DismissedByUser,
                    )
                    .await
                    {
                        let mut first = first_error
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        record_first_error(&mut first, error);
                    }
                }
            })
            .await;

        first_error
            .into_inner()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .map_or(Ok(()), Err)
    }
}
