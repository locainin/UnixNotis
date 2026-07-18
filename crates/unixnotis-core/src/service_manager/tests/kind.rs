use super::super::ServiceManagerKind;

#[test]
fn service_manager_parser_accepts_supported_aliases_and_rejects_unknown_values() {
    assert_eq!(
        ServiceManagerKind::parse("systemd-user").expect("systemd alias"),
        ServiceManagerKind::Systemd
    );
    assert_eq!(
        ServiceManagerKind::parse("s6-user").expect("s6 alias"),
        ServiceManagerKind::S6
    );
    assert!(ServiceManagerKind::parse("").is_err());
    assert!(ServiceManagerKind::parse("openrc").is_err());
    assert_eq!(
        "runit"
            .parse::<ServiceManagerKind>()
            .expect("FromStr runit"),
        ServiceManagerKind::Runit
    );
}

#[test]
fn all_service_managers_have_distinct_stable_labels() {
    let labels = ServiceManagerKind::all()
        .into_iter()
        .map(ServiceManagerKind::label)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(labels.len(), ServiceManagerKind::all().len());
}
