//! Human-readable notification attribution and popup diagnostics

use std::fmt::Write;

use anyhow::Result;
use unixnotis_core::{
    CommandLineQualityView, LaunchAuthorityView, LaunchVerificationView,
    NotificationDiagnosticsView, PopupAdmissionView, RecordTrust,
};

use super::write_stdout;

pub fn print_notification_diagnostics(view: &NotificationDiagnosticsView) -> Result<()> {
    let diagnostics = &view.attribution;
    let mut output = String::new();
    writeln!(output, "Notification: {}:{}", view.id, view.generation)?;
    writeln!(
        output,
        "Application claim: {}",
        value_or_none(&diagnostics.claimed_name)
    )?;
    writeln!(
        output,
        "Claimed desktop entry: {}",
        value_or_none(&diagnostics.claimed_desktop_entry)
    )?;
    writeln!(
        output,
        "Sender executable: {}",
        value_or_none(&diagnostics.sender_executable)
    )?;
    writeln!(
        output,
        "Matched desktop ID: {}",
        value_or_none(&diagnostics.matched_desktop_id)
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
        "Identity result: {}",
        verification(diagnostics.verification)
    )?;
    writeln!(
        output,
        "Identity reason: {}",
        value_or_none(&diagnostics.reason)
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
    write_stdout(&output)
}

fn value_or_none(value: &str) -> &str {
    if value.trim().is_empty() {
        "none"
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
