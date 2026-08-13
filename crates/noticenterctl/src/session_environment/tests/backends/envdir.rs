use super::super::super::backends::envdir::write_envdir;
use super::super::support::TempToolDir;
use unixnotis_core::service_manager::ServiceManagerKind;

#[test]
fn envdir_writer_rejects_a_non_directory_service_anchor() {
    let root = TempToolDir::new("envdir-anchor");
    let service = root.write_file("unixnotis-daemon", "not a directory");

    let error = write_envdir(
        &service,
        &root.path().join("env"),
        ServiceManagerKind::Runit,
    )
    .expect_err("non-directory service must be rejected");

    assert!(error.to_string().contains("regular service directory"));
}
