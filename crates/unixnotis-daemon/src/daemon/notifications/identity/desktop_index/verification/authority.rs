//! Launch-authority classification

use super::super::model::{
    DesktopIdentityIndex, DesktopRecord, LaunchArgument, LaunchAuthority, LaunchSpec,
    LiteralArgument,
};

pub(super) fn classify_launch_authority(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    spec: &LaunchSpec,
) -> LaunchAuthority {
    if spec.arguments.iter().any(is_protected_payload) {
        return LaunchAuthority::ProtectedPayload;
    }

    if executable_contract_is_dedicated(record, index, spec) {
        return LaunchAuthority::DedicatedExecutable;
    }

    // Dynamic documents are safe only after the executable establishes the application
    if spec.arguments.iter().any(is_dynamic_document_field) {
        return LaunchAuthority::DynamicOnly;
    }

    LaunchAuthority::Ambiguous
}

pub(super) fn executable_contract_is_dedicated(
    record: &DesktopRecord,
    index: &DesktopIdentityIndex,
    spec: &LaunchSpec,
) -> bool {
    record.system_origin
        && record.system_association
        && spec.declared_executable.is_system_managed()
        && spec.declared_executable.is_executable_regular()
        && spec.runtime_executable.is_system_managed()
        && spec.runtime_executable.is_executable_regular()
        && record
            .desktop_provenance
            .same_application_source(&record.declared_executable_provenance)
        && record
            .desktop_provenance
            .same_application_source(&record.runtime_executable_provenance)
        && index.records_form_one_application_family(spec.runtime_executable, record.system_origin)
        && !spec.arguments.iter().any(is_unprotected_fixed_payload)
}
pub(super) fn is_protected_payload(argument: &LaunchArgument) -> bool {
    matches!(
        argument,
        LaunchArgument::Literal(LiteralArgument {
            file: Some(_),
            value,
        }) if !value.starts_with(b"-")
    )
}

pub(super) const fn is_dynamic_document_field(argument: &LaunchArgument) -> bool {
    matches!(argument, LaunchArgument::FieldCode(_))
}

pub(super) fn is_unprotected_fixed_payload(argument: &LaunchArgument) -> bool {
    matches!(
        argument,
        LaunchArgument::Literal(literal)
            if !literal.value.starts_with(b"-") && literal.file.is_none()
    )
}
