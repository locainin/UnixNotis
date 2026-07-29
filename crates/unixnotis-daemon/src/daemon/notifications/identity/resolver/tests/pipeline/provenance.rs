//! Async provenance enrichment in the resolver pipeline

use super::super::*;

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
    record.executable_provenance = app_provenance;
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
