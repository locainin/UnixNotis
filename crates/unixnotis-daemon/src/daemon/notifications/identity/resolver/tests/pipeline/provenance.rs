//! Async provenance enrichment in the resolver pipeline

use super::super::*;

#[test]
fn provenance_enrichment_is_limited_to_denied_association_candidates() {
    for (status, policies, has_candidate, expected) in [
        (
            AttributionStatus::Recognized,
            InteractionPolicies::DENY,
            false,
            true,
        ),
        (
            AttributionStatus::Unresolved,
            InteractionPolicies::DENY,
            true,
            true,
        ),
        (
            AttributionStatus::Unresolved,
            InteractionPolicies::DENY,
            false,
            false,
        ),
        (
            AttributionStatus::Recognized,
            InteractionPolicies::NATIVE_COMPATIBILITY,
            true,
            false,
        ),
        (
            AttributionStatus::Conflict,
            InteractionPolicies::DENY,
            true,
            false,
        ),
    ] {
        assert_eq!(
            needs_sender_provenance(status, policies, has_candidate),
            expected,
            "status={status:?}, policies={policies:?}, has_candidate={has_candidate}"
        );
    }
}

#[test]
fn provenance_candidate_lookup_accepts_only_indexed_name_or_desktop_id() {
    let record = system_record(
        "org.example.App",
        "Example App",
        "/usr/bin/example-app",
        identity(41, 42, 0),
    );
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());

    assert!(claim_has_index_candidate(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: None,
        },
        &index,
    ));
    assert!(claim_has_index_candidate(
        AppClaim {
            reported_name: "",
            desktop_entry: Some("org.example.App"),
        },
        &index,
    ));
    assert!(!claim_has_index_candidate(
        AppClaim {
            reported_name: "Unknown App",
            desktop_entry: Some("org.example.Missing"),
        },
        &index,
    ));
}

#[tokio::test]
async fn recognized_helper_is_reresolved_with_live_package_provenance() {
    let helper_path = unixnotis_core::util::trusted_system_program_path("true")
        .expect("find the installed helper fixture");
    let app_path = unixnotis_core::util::trusted_system_program_path("false")
        .expect("find the installed application fixture");
    let helper_evidence =
        executable_evidence_for_path(&helper_path).expect("read the helper executable identity");
    let app_evidence =
        executable_evidence_for_path(&app_path).expect("read the application executable identity");
    let ownership_index = DesktopIdentityIndex::default();
    let helper_provenance = ownership_index
        .install_provenance_for_path_async(helper_path.clone())
        .await;
    let app_provenance = ownership_index
        .install_provenance_for_path_async(app_path.clone())
        .await;
    assert!(helper_provenance.is_known());
    assert!(helper_provenance.same_application_source(&app_provenance));

    let mut record = system_record(
        "org.example.App",
        "Example App",
        &app_path.display().to_string(),
        app_evidence.identity,
    );
    record.desktop_provenance = app_provenance.clone();
    record.declared_executable_provenance = app_provenance.clone();
    record.runtime_executable_provenance = app_provenance;
    let index = DesktopIdentityIndex::from_records(vec![record], Vec::new());
    let resolution = resolve_attribution(
        AppClaim {
            reported_name: "Example App",
            desktop_entry: Some("org.example.App"),
        },
        &sender(&helper_path.display().to_string(), helper_evidence.identity),
        &index,
    )
    .await;

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert!(resolution
        .attribution
        .diagnostic_detail
        .contains("same installed application package"));
}
