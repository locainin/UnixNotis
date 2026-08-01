//! Async provenance enrichment in the resolver pipeline

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use std::time::Duration;

use super::super::*;

#[test]
fn initial_resolution_skips_provenance_when_no_lookup_is_needed() {
    assert!(should_return_initial_resolution(false));
    assert!(!should_return_initial_resolution(true));
}

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
    let helper_provenance = ownership_index.install_provenance_for_path(helper_path.clone());
    let app_provenance = ownership_index.install_provenance_for_path(app_path.clone());
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

#[tokio::test]
async fn slow_valid_provenance_is_not_cut_off_by_a_short_inner_deadline() {
    let helper_path = unixnotis_core::util::trusted_system_program_path("true")
        .expect("find the installed helper fixture");
    let app_path = unixnotis_core::util::trusted_system_program_path("false")
        .expect("find the installed application fixture");
    let helper_evidence =
        executable_evidence_for_path(&helper_path).expect("read helper executable evidence");
    let app_evidence =
        executable_evidence_for_path(&app_path).expect("read application executable evidence");
    let ownership_index = DesktopIdentityIndex::default();
    let app_provenance = ownership_index.install_provenance_for_path(app_path.clone());
    assert!(app_provenance.is_known());

    let mut record = system_record(
        "org.example.App",
        "Example App",
        &app_path.display().to_string(),
        app_evidence.identity,
    );
    record.desktop_provenance = app_provenance.clone();
    record.declared_executable_provenance = app_provenance.clone();
    record.runtime_executable_provenance = app_provenance.clone();
    let index = Arc::new(DesktopIdentityIndex::from_records(vec![record], Vec::new()));

    let resolution = resolve_attribution_owned_with(
        "Example App".to_string(),
        Some("org.example.App".to_string()),
        sender(&helper_path.display().to_string(), helper_evidence.identity),
        index,
        move |sender, _| {
            // Package ownership can exceed the old 500 ms inner deadline
            std::thread::sleep(Duration::from_millis(650));
            sender.install_provenance = app_provenance;
        },
    )
    .await;

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(
        resolution.attribution.interactions,
        InteractionPolicies::DENY
    );
}

#[tokio::test]
async fn failed_provenance_keeps_the_initial_safe_resolution() {
    let helper_path = unixnotis_core::util::trusted_system_program_path("true")
        .expect("find the installed helper fixture");
    let app_path = unixnotis_core::util::trusted_system_program_path("false")
        .expect("find the installed application fixture");
    let helper_evidence =
        executable_evidence_for_path(&helper_path).expect("read helper executable evidence");
    let app_evidence =
        executable_evidence_for_path(&app_path).expect("read application executable evidence");
    let ownership_index = DesktopIdentityIndex::default();
    let app_provenance = ownership_index.install_provenance_for_path(app_path.clone());
    assert!(app_provenance.is_known());

    let mut record = system_record(
        "org.example.App",
        "Example App",
        &app_path.display().to_string(),
        app_evidence.identity,
    );
    record.desktop_provenance = app_provenance.clone();
    record.declared_executable_provenance = app_provenance.clone();
    record.runtime_executable_provenance = app_provenance.clone();
    let index = Arc::new(DesktopIdentityIndex::from_records(vec![record], Vec::new()));
    let mut sender = sender(&helper_path.display().to_string(), helper_evidence.identity);
    sender.install_provenance = app_provenance;

    let resolution = resolve_attribution_owned_with(
        "Example App".to_string(),
        Some("org.example.App".to_string()),
        sender,
        index,
        |sender, _| {
            // Model a provider failure without granting a stronger result
            sender.install_provenance = InstallProvenance::Unknown;
        },
    )
    .await;

    assert_eq!(resolution.attribution.status, AttributionStatus::Recognized);
    assert_eq!(
        resolution.attribution.interactions,
        InteractionPolicies::DENY
    );
}

#[tokio::test]
async fn ingress_deadline_fails_closed_after_the_real_resolver_exceeds_budget() {
    let (index, sender) = same_package_helper_fixture();
    let slow_resolution = async {
        let resolution = resolve_attribution_owned_with(
            "Example App".to_string(),
            Some("org.example.App".to_string()),
            sender.clone(),
            index,
            move |_, _| {
                std::thread::sleep(ATTRIBUTION_TIMEOUT + Duration::from_millis(250));
            },
        )
        .await;
        // Keep the injected production future beyond the outer ingress budget
        tokio::time::sleep(ATTRIBUTION_TIMEOUT + Duration::from_millis(250)).await;
        resolution
    };
    let resolution = resolve_attribution_with_deadline(
        "Example App".to_string(),
        Some("org.example.App".to_string()),
        &sender,
        slow_resolution,
    )
    .await;

    assert_eq!(resolution.attribution.status, AttributionStatus::Unresolved);
    assert_eq!(
        resolution.attribution.interactions,
        InteractionPolicies::DENY
    );
    assert!(resolution
        .attribution
        .diagnostic_detail
        .contains("attribution timed out"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelled_attribution_keeps_worker_permits_until_blocking_jobs_exit() {
    let (index, sender) = same_package_helper_fixture();
    let worker_pool = Arc::new(tokio::sync::Semaphore::new(8));
    let started = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicUsize::new(0));
    let release = Arc::new(Barrier::new(9));
    let mut tasks = Vec::with_capacity(8);

    for _ in 0..8 {
        let index = Arc::clone(&index);
        let sender = sender.clone();
        let worker_pool = Arc::clone(&worker_pool);
        let started = Arc::clone(&started);
        let finished = Arc::clone(&finished);
        let release = Arc::clone(&release);
        tasks.push(tokio::spawn(resolve_attribution_owned_with_pool(
            "Example App".to_string(),
            Some("org.example.App".to_string()),
            sender,
            index,
            worker_pool,
            move |_, _| {
                started.fetch_add(1, Ordering::AcqRel);
                release.wait();
                finished.fetch_add(1, Ordering::Release);
            },
        )));
    }

    tokio::time::timeout(Duration::from_secs(2), async {
        while started.load(Ordering::Acquire) != 8 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("all attribution workers should enter blocking enrichment");

    for task in &tasks {
        task.abort();
    }

    let blocked = resolve_attribution_owned_with_pool(
        "Example App".to_string(),
        Some("org.example.App".to_string()),
        sender.clone(),
        Arc::clone(&index),
        Arc::clone(&worker_pool),
        |_, _| {},
    )
    .await;
    assert!(blocked
        .attribution
        .diagnostic_detail
        .contains("attribution worker capacity exhausted"));

    release.wait();
    tokio::time::timeout(Duration::from_secs(2), async {
        while finished.load(Ordering::Acquire) != 8 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker permits should be released after blocking jobs exit");

    let available = resolve_attribution_owned_with_pool(
        "Example App".to_string(),
        Some("org.example.App".to_string()),
        sender,
        index,
        worker_pool,
        |_, _| {},
    )
    .await;
    assert!(!available
        .attribution
        .diagnostic_detail
        .contains("attribution worker capacity exhausted"));
}

fn same_package_helper_fixture() -> (Arc<DesktopIdentityIndex>, SenderMetadata) {
    let helper_path = unixnotis_core::util::trusted_system_program_path("true")
        .expect("find the installed helper fixture");
    let app_path = unixnotis_core::util::trusted_system_program_path("false")
        .expect("find the installed application fixture");
    let helper_evidence =
        executable_evidence_for_path(&helper_path).expect("read helper executable evidence");
    let app_evidence =
        executable_evidence_for_path(&app_path).expect("read application executable evidence");
    let ownership_index = DesktopIdentityIndex::default();
    let app_provenance = ownership_index.install_provenance_for_path(app_path.clone());
    assert!(app_provenance.is_known());

    let mut record = system_record(
        "org.example.App",
        "Example App",
        &app_path.display().to_string(),
        app_evidence.identity,
    );
    record.desktop_provenance = app_provenance.clone();
    record.declared_executable_provenance = app_provenance.clone();
    record.runtime_executable_provenance = app_provenance;

    (
        Arc::new(DesktopIdentityIndex::from_records(vec![record], Vec::new())),
        sender(&helper_path.display().to_string(), helper_evidence.identity),
    )
}
