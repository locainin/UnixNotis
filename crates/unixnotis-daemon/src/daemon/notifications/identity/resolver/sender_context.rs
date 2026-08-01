//! Live sender metadata used by the attribution pipeline

use super::super::desktop_index::DesktopIdentityIndex;
use super::super::executable::executable_evidence_for_path;
use super::super::sender::SenderMetadata;
use super::evidence::current_system_identity_matches_sender;

pub(super) fn enrich_sender_install_provenance_blocking(
    sender: &mut SenderMetadata,
    index: &DesktopIdentityIndex,
) {
    if sender.install_provenance.is_known() {
        return;
    }
    let (Some(path), Some(sender_identity)) = (
        sender.sender_executable.as_deref(),
        sender.sender_executable_identity,
    ) else {
        return;
    };
    if !sender_identity.is_system_managed() || !sender_identity.is_executable_regular() {
        return;
    }

    // Reopen the executable before package ownership can affect attribution
    let Some(current) = executable_evidence_for_path(std::path::Path::new(path)) else {
        return;
    };
    if !current_system_identity_matches_sender(current.identity, sender_identity) {
        return;
    }
    sender.install_provenance = index.install_provenance_for_path(current.canonical_path);
}

#[cfg_attr(
    not(test),
    expect(dead_code, reason = "async wrapper remains for resolver tests")
)]
pub(super) async fn enrich_sender_install_provenance(
    sender: &mut SenderMetadata,
    index: &DesktopIdentityIndex,
) {
    enrich_sender_install_provenance_blocking(sender, index);
}
