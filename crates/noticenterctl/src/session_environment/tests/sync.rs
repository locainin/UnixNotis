use super::super::sync;
use crate::cli::DoctorServiceManagerArg;

#[test]
fn top_level_sync_rejects_manual_service_management() {
    let error = sync(DoctorServiceManagerArg::Manual)
        .expect_err("manual service management cannot be synchronized");

    assert!(!error.to_string().is_empty());
}
