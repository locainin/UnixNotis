//! Built-in reader and worker tests

use super::super::builtin::readers::battery::read_battery_from;
use super::super::builtin::readers::network::{pick_default_iface_from, IfaceCandidate};
use super::super::builtin::worker::{BuiltinJob, BuiltinSample, BuiltinWorker, SubmitOutcome};
use super::super::builtin::BuiltinStat;
use super::support::{write_device, TempDir};

#[test]
fn battery_energy_values_are_weighted_by_full_capacity() {
    let temp = TempDir::new("unixnotis-battery-energy");
    write_device(
        temp.path(),
        "BAT0",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("energy_now", "30"),
            ("energy_full", "60"),
        ],
    );
    write_device(
        temp.path(),
        "BAT1",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("energy_now", "10"),
            ("energy_full", "40"),
        ],
    );

    let percent = read_battery_from(temp.path()).expect("battery percent missing");

    assert_eq!(percent, "40");
}

#[test]
fn battery_mixed_units_fall_back_to_reported_capacity() {
    let temp = TempDir::new("unixnotis-battery-mixed");
    write_device(
        temp.path(),
        "BAT0",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("energy_now", "30"),
            ("energy_full", "60"),
            ("capacity", "60"),
        ],
    );
    write_device(
        temp.path(),
        "BAT1",
        &[
            ("type", "Battery"),
            ("present", "1"),
            ("charge_now", "10"),
            ("charge_full", "40"),
            ("capacity", "25"),
        ],
    );

    let percent = read_battery_from(temp.path()).expect("battery percent missing");

    assert_eq!(percent, "43");
}

#[test]
fn battery_reader_skips_devices_reported_as_absent() {
    let temp = TempDir::new("unixnotis-battery-absent");
    write_device(
        temp.path(),
        "BAT0",
        &[
            ("type", "Battery"),
            ("present", "0"),
            ("energy_now", "30"),
            ("energy_full", "60"),
        ],
    );

    assert!(read_battery_from(temp.path()).is_none());
}

#[test]
fn default_interface_prefers_an_active_physical_device() {
    let candidates = vec![
        IfaceCandidate {
            name: "veth0".to_string(),
            operstate: "up".to_string(),
        },
        IfaceCandidate {
            name: "wlan0".to_string(),
            operstate: "up".to_string(),
        },
    ];

    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("wlan0".to_string())
    );
}

#[test]
fn default_interface_prefers_physical_devices_when_all_are_down() {
    let candidates = vec![
        IfaceCandidate {
            name: "eth0".to_string(),
            operstate: "down".to_string(),
        },
        IfaceCandidate {
            name: "docker0".to_string(),
            operstate: "up".to_string(),
        },
    ];

    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("eth0".to_string())
    );
}

#[test]
fn default_interface_uses_name_as_a_deterministic_tiebreaker() {
    let candidates = vec![
        IfaceCandidate {
            name: "eth1".to_string(),
            operstate: "down".to_string(),
        },
        IfaceCandidate {
            name: "eth0".to_string(),
            operstate: "down".to_string(),
        },
    ];

    assert_eq!(
        pick_default_iface_from(&candidates),
        Some("eth0".to_string())
    );
}

#[test]
fn builtin_worker_reports_a_full_queue_without_blocking() {
    let (tx, _worker_rx) = crossbeam_channel::bounded(1);
    let worker = BuiltinWorker {
        tx,
        inline_fallback: false,
    };
    let first = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let second = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let (first_tx, _first_rx) = async_channel::bounded(1);
    let (second_tx, _second_rx) = async_channel::bounded(1);

    assert_eq!(
        worker.submit(BuiltinJob {
            stat: first,
            respond: first_tx,
        }),
        SubmitOutcome::Submitted
    );
    assert_eq!(
        worker.submit(BuiltinJob {
            stat: second,
            respond: second_tx,
        }),
        SubmitOutcome::QueueFull
    );
}

#[test]
fn builtin_sample_preserves_reader_failure_as_missing_data() {
    let stat =
        BuiltinStat::from_command("builtin:net:unixnotis-missing-interface").expect("builtin stat");

    let sample = BuiltinSample::read(stat);

    assert!(sample.value.is_none());
}
