use super::DaemonActivationReservation;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct TestReservationBacking {
    observer: Arc<AtomicBool>,
}

impl super::ReservationBacking for TestReservationBacking {}

impl Drop for TestReservationBacking {
    fn drop(&mut self) {
        self.observer.store(false, Ordering::Release);
    }
}

impl DaemonActivationReservation {
    pub(crate) fn test_guard(observer: Arc<AtomicBool>) -> Self {
        observer.store(true, Ordering::Release);
        Self {
            backing: Box::new(TestReservationBacking { observer }),
        }
    }
}

fn acquire_name(name: &str) -> anyhow::Result<DaemonActivationReservation> {
    DaemonActivationReservation::acquire_names(&[name])
}

#[test]
fn reservation_excludes_another_connection_until_the_guard_drops() {
    let name = format!(
        "io.github.unixnotis.InstallerReservation{}",
        std::process::id()
    );
    let first =
        acquire_name(&name).expect("first connection should reserve the isolated test name");

    let error = match acquire_name(&name) {
        Ok(_unexpected) => panic!("a second connection acquired the reserved test name"),
        Err(error) => error,
    };

    assert!(
        error.to_string().contains("D-Bus activation name"),
        "unexpected competing reservation error: {error:#}"
    );
    drop(first);
    acquire_name(&name).expect("the name should become available after the guard drops");
}

#[test]
fn reservation_blocks_both_activation_names_until_the_guard_drops() {
    let suffix = std::process::id();
    let notifications = format!("io.github.unixnotis.InstallerNotificationsReservation{suffix}");
    let control = format!("io.github.unixnotis.InstallerControlReservation{suffix}");
    let first = DaemonActivationReservation::acquire_names(&[&notifications, &control])
        .expect("one connection should reserve both activation names");

    for name in [&notifications, &control] {
        let error = match acquire_name(name) {
            Ok(_unexpected) => {
                panic!("a second connection acquired a reserved activation name")
            }
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("D-Bus activation name"),
            "unexpected competing reservation error: {error:#}"
        );
    }

    drop(first);
    DaemonActivationReservation::acquire_names(&[&notifications, &control])
        .expect("both names should become available after the guard drops");
}

#[test]
fn failed_second_name_request_releases_the_first_name() {
    let suffix = std::process::id();
    let occupied = format!("io.github.unixnotis.OccupiedReservation{suffix}");
    let released = format!("io.github.unixnotis.ReleasedReservation{suffix}");
    let owner =
        acquire_name(&occupied).expect("the competing connection should reserve the second name");

    let error = match DaemonActivationReservation::acquire_names(&[&released, &occupied]) {
        Ok(_unexpected) => panic!("a reservation succeeded after its second name was taken"),
        Err(error) => error,
    };
    assert!(
        error.to_string().contains("D-Bus activation name"),
        "unexpected partial reservation error: {error:#}"
    );

    acquire_name(&released).expect("the first name must be released when the second request fails");
    drop(owner);
}
