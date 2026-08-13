use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use unixnotis_core::{
    Config, Notification, NotificationKey, PopupAdmissionView, PopupDecisionRecord,
};

use super::dnd::DndStateStore;
use super::inhibitors::Inhibitor;
use super::notifications::HistoryStore;

/// Mutable notification state owned by the daemon
pub struct NotificationStore {
    // Immutable runtime config snapshot
    pub(super) config: Config,
    // Next candidate id for allocation
    pub(super) next_id: u32,
    // Commit generations never reuse identity when replacements preserve an ID
    pub(super) next_generation: u64,
    // Active notifications in insertion order
    pub(super) active: IndexMap<u32, Arc<Notification>>,
    // Archived notifications with bounded retention
    pub(super) history: HistoryStore,
    // Arrival-time popup decisions outlive active state while history retains the generation
    pub(super) popup_decisions: HashMap<NotificationKey, PopupDecisionRecord>,
    // Monotonic popup deadlines stay daemon-local and are never serialized
    pub(super) popup_timings: HashMap<NotificationKey, PopupTiming>,
    // Exact expiration identity per active notification generation
    pub(super) expirations: HashMap<u32, ExpirationTicket>,
    // Effective DND switch after loading persisted state
    pub(super) dnd_enabled: bool,
    // Wall-clock deadline survives daemon restarts; None means indefinite
    pub(super) dnd_expires_at: Option<i64>,
    // Monotonic in-memory revision for DND writes
    pub(super) dnd_revision: u64,
    // Optional persistence layer for DND; absent store keeps behavior in-memory
    pub(super) dnd_state_store: Option<DndStateStore>,
    // Token counter for inhibitors; never reused in a process
    pub(super) next_inhibitor_id: u64,
    // Active inhibitors keyed by token for quick lookup/removal
    pub(super) inhibitors: HashMap<u64, Inhibitor>,
    // Cached flags avoid rescanning inhibitors on every notification
    pub(super) inhibited: bool,
    pub(super) inhibitor_count: u32,
}

pub struct InsertOutcome {
    // Commit kind keeps content-bearing and content-free lifecycles structurally distinct
    pub disposition: CommitDisposition,
    // True when insertion replaced an existing id
    pub replaced: bool,
    // Structured popup policy keeps suppression causes available to diagnostics
    pub popup_admission: PopupAdmission,
    // Whether sound playback is allowed for this payload
    pub allow_sound: bool,
    // Active ids evicted because max_active was exceeded
    pub evicted: Vec<NotificationKey>,
    // Commit-time daemon deadline for this exact generation
    pub expiration: Option<Instant>,
}

/// Result of committing one protocol notification request
pub enum CommitDisposition {
    // Ordinary notifications retain their normalized content in active storage
    Active(Arc<Notification>),
    // DropAll retains no sender-controlled presentation content
    SuppressedDropAll(SuppressedNotification),
}

/// Content-free lifecycle identity for a `DropAll` notification
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SuppressedNotification {
    pub id: u32,
    pub generation: u64,
    // Stable process identity is retained only when both components are established
    pub owner: Option<StableProcessIdentity>,
}

/// Process-lifetime ownership principal independent of a D-Bus unique name
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StableProcessIdentity {
    pub pid: u32,
    pub start_time: u64,
}

/// Deliberately collapsed result of authorizing a protocol close request
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseAuthorization {
    OwnedActive(NotificationKey),
    NotClosable,
}

impl InsertOutcome {
    pub const fn suppressed(&self) -> Option<SuppressedNotification> {
        match &self.disposition {
            CommitDisposition::Active(_) => None,
            CommitDisposition::SuppressedDropAll(suppressed) => Some(*suppressed),
        }
    }
}

/// Daemon-only popup lifetime for one committed generation
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PopupTiming {
    // None keeps the popup eligible until delivery because zero disables automatic hiding
    pub(super) deadline: Option<Instant>,
}

/// Exact identity required to expire one committed notification
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpirationTicket {
    pub id: u32,
    pub generation: u64,
    pub deadline: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PopupAdmission {
    Show,
    Suppressed(PopupSuppressionReason),
}

impl PopupAdmission {
    pub const fn should_show(self) -> bool {
        matches!(self, Self::Show)
    }

    pub const fn to_view(self) -> PopupAdmissionView {
        match self {
            Self::Show => PopupAdmissionView::Show,
            Self::Suppressed(PopupSuppressionReason::Rule) => PopupAdmissionView::Rule,
            Self::Suppressed(PopupSuppressionReason::Dnd) => PopupAdmissionView::Dnd,
            Self::Suppressed(
                PopupSuppressionReason::Inhibitor | PopupSuppressionReason::DropAllInhibitor,
            ) => PopupAdmissionView::Inhibitor,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PopupSuppressionReason {
    Rule,
    Dnd,
    Inhibitor,
    DropAllInhibitor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryStageUpdate {
    Advanced,
    AlreadyAtOrBeyond,
    MissingGeneration,
}

pub struct DndWrite {
    // True when the in-memory DND value changed
    pub(crate) changed: bool,
    // Value seen before this write
    pub(crate) previous: bool,
    // Deadline paired with the previous switch value
    pub(crate) previous_expires_at: Option<i64>,
    // Value written by this operation
    pub(crate) current: bool,
    // Deadline paired with the current switch value
    pub(crate) current_expires_at: Option<i64>,
    // Monotonic revision captured for guarded rollback
    pub(crate) revision: u64,
    // Persistence backend used outside the store lock
    pub(crate) persist: Option<DndStateStore>,
}

pub struct DismissOutcome {
    // Exact active generation removed by the operation
    pub removed_active: Option<NotificationKey>,
    // Exact history generation removed by the operation
    pub removed_history: Option<NotificationKey>,
}

impl DismissOutcome {
    pub const fn removed_any(&self) -> bool {
        // Convenience helper for callers that only need yes/no
        self.removed_active.is_some() || self.removed_history.is_some()
    }
}
