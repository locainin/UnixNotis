//! Human-readable notification attribution and popup diagnostics

use std::fmt::Write;

use anyhow::Result;
use unixnotis_core::{
    util, ApplicationActionPolicy, CommandLineQualityView, IdentityAssurance, InlineReplyPolicy,
    LaunchAuthorityView, LaunchVerificationView, NotificationDiagnosticsView, PopupAdmissionView,
    PopupDeliveryStage, RecordTrust,
};

use super::write_stdout;

pub fn print_notification_diagnostics(view: &NotificationDiagnosticsView) -> Result<()> {
    write_stdout(&format_notification_diagnostics(view)?)
}

// Diagnostic wire values include sender-controlled notification metadata
// Every free-form string passes through the terminal sanitizer here
// Enum labels and numeric fields cannot carry free-form terminal text
fn format_notification_diagnostics(view: &NotificationDiagnosticsView) -> Result<String> {
    let diagnostics = &view.attribution;
    let mut output = String::new();
    writeln!(output, "Notification: {}:{}", view.id, view.generation)?;
    writeln!(
        output,
        "Application claim: {}",
        diagnostic_value(&diagnostics.claimed_name)
    )?;
    writeln!(
        output,
        "Claimed desktop entry: {}",
        diagnostic_value(&diagnostics.claimed_desktop_entry)
    )?;
    writeln!(
        output,
        "Sender executable: {}",
        diagnostic_value(&diagnostics.sender_executable)
    )?;
    writeln!(
        output,
        "Matched desktop ID: {}",
        diagnostic_value(&diagnostics.matched_desktop_id)
    )?;
    writeln!(
        output,
        "Record origin: {}",
        record_trust(diagnostics.record_trust)
    )?;
    writeln!(
        output,
        "Launch authority: {}",
        launch_authority(diagnostics.launch_authority)
    )?;
    writeln!(
        output,
        "Command line: {}",
        command_line_quality(diagnostics.command_line_quality)
    )?;
    writeln!(
        output,
        "Launch verification: {}",
        verification(diagnostics.verification)
    )?;
    writeln!(
        output,
        "Launch detail: {}",
        diagnostic_value(&diagnostics.reason)
    )?;
    writeln!(
        output,
        "Identity assurance: {}",
        identity_assurance(view.identity_assurance)
    )?;
    writeln!(
        output,
        "Default activation: {}",
        action_policy(view.interaction_policies.default_activation)
    )?;
    writeln!(
        output,
        "Action buttons: {}",
        action_policy(view.interaction_policies.action_buttons)
    )?;
    writeln!(
        output,
        "Inline reply: {}",
        reply_policy(view.interaction_policies.inline_reply)
    )?;
    writeln!(output, "Stored: {}", yes_no(view.stored))?;
    writeln!(
        output,
        "Popup: {}",
        if view.popup_admission.should_show() {
            "allowed"
        } else {
            "suppressed"
        }
    )?;
    writeln!(
        output,
        "Popup reason: {}",
        popup_admission(view.popup_admission)
    )?;
    writeln!(
        output,
        "Renderer process: {}",
        if view.renderer_process_running {
            "running"
        } else {
            "unavailable"
        }
    )?;
    writeln!(output, "Renderer ready: {}", yes_no(view.renderer_ready))?;
    writeln!(
        output,
        "Configured max visible: {}",
        view.configured_max_visible
    )?;
    writeln!(
        output,
        "Decision time (Unix ms): {}",
        view.decided_at_unix_ms
    )?;
    writeln!(
        output,
        "Delivery stage: {}",
        popup_delivery_stage(view.delivery_stage)
    )?;
    Ok(output)
}

const fn identity_assurance(value: IdentityAssurance) -> &'static str {
    match value {
        IdentityAssurance::Authenticated => "authenticated",
        IdentityAssurance::SystemAssociated => "system associated",
        IdentityAssurance::PortalAssociated => "portal associated",
        IdentityAssurance::UserAssociated => "user associated",
        IdentityAssurance::Unresolved => "unresolved",
        IdentityAssurance::Conflict => "conflict",
        IdentityAssurance::Relay => "relay",
    }
}

const fn action_policy(value: ApplicationActionPolicy) -> &'static str {
    match value {
        ApplicationActionPolicy::Allow => "allowed",
        ApplicationActionPolicy::Confirm => "confirmation required",
        ApplicationActionPolicy::Deny => "denied",
    }
}

const fn reply_policy(value: InlineReplyPolicy) -> &'static str {
    match value {
        InlineReplyPolicy::Allow => "allowed",
        InlineReplyPolicy::Confirm => "confirmation required",
        InlineReplyPolicy::Deny => "denied",
    }
}

const fn popup_delivery_stage(value: PopupDeliveryStage) -> &'static str {
    match value {
        PopupDeliveryStage::Suppressed => "suppressed",
        PopupDeliveryStage::Admitted => "admitted",
        PopupDeliveryStage::FanoutFailed => "fanout failed",
        PopupDeliveryStage::RendererFetched => "renderer fetched",
        PopupDeliveryStage::Materialized => "materialized",
        PopupDeliveryStage::Visible => "visible",
    }
}

fn diagnostic_value(value: &str) -> String {
    // Attribution diagnostics may contain sender-controlled metadata
    // Keep each field bounded and single-line before it reaches the terminal
    let value = util::sanitize_log_value(value, util::diagnostic_log_limit());

    if value.is_empty() {
        "none".to_string()
    } else {
        value
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

const fn record_trust(value: RecordTrust) -> &'static str {
    match value {
        RecordTrust::None => "none",
        RecordTrust::Portal => "portal",
        RecordTrust::System => "system",
        RecordTrust::User => "user",
    }
}

const fn launch_authority(value: LaunchAuthorityView) -> &'static str {
    match value {
        LaunchAuthorityView::None => "none",
        LaunchAuthorityView::DedicatedExecutable => "dedicated executable",
        LaunchAuthorityView::ProtectedPayload => "protected payload",
        LaunchAuthorityView::DynamicOnly => "dynamic-only contract",
        LaunchAuthorityView::Ambiguous => "ambiguous",
    }
}

const fn command_line_quality(value: CommandLineQualityView) -> &'static str {
    match value {
        CommandLineQualityView::Structured => "structured",
        CommandLineQualityView::RewrittenProcessTitle => "rewritten process title",
        CommandLineQualityView::Truncated => "truncated",
        CommandLineQualityView::Unavailable => "unavailable",
    }
}

const fn verification(value: LaunchVerificationView) -> &'static str {
    match value {
        LaunchVerificationView::Verified => "verified",
        LaunchVerificationView::InsufficientEvidence => "unverified",
        LaunchVerificationView::DefinitiveMismatch => "suspicious",
    }
}

const fn popup_admission(value: PopupAdmissionView) -> &'static str {
    match value {
        PopupAdmissionView::Show => "show",
        PopupAdmissionView::Rule => "rule",
        PopupAdmissionView::Dnd => "DND",
        PopupAdmissionView::Inhibitor => "inhibitor",
        PopupAdmissionView::RendererUnavailable => "renderer unavailable",
        PopupAdmissionView::RendererDisabled => "renderer disabled",
    }
}

#[cfg(test)]
#[path = "tests/diagnostics.rs"]
mod tests;
