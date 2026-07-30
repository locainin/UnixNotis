//! Read-only notification explanation returned by the control service

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

use crate::{AttributionDiagnostics, IdentityAssurance, InteractionPolicies};

use super::{PopupAdmissionView, PopupDeliveryStage};

/// One active notification and the state that controls its popup rendering
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct NotificationDiagnosticsView {
    pub id: u32,
    pub generation: u64,
    pub stored: bool,
    pub attribution: AttributionDiagnostics,
    // Final authority stays separate from the lower-level launch evidence above
    pub identity_assurance: IdentityAssurance,
    pub interaction_policies: InteractionPolicies,
    pub popup_admission: PopupAdmissionView,
    pub renderer_process_running: bool,
    pub renderer_ready: bool,
    pub configured_max_visible: u32,
    pub decided_at_unix_ms: i64,
    pub delivery_stage: PopupDeliveryStage,
}
