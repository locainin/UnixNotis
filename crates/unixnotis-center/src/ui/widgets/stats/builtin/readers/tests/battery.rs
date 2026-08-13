//! Battery reader tests

use super::super::tests::support::{write_device, TempDir};
use super::read_battery_from;

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
