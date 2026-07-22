//! Built-in statistic identity tests

use super::super::{BuiltinStat, BuiltinStatKey};

#[test]
fn matching_builtin_sources_produce_the_same_group_key() {
    let first = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");
    let second = BuiltinStat::from_command("builtin:cpu").expect("builtin stat");

    assert_eq!(first.key(), BuiltinStatKey::Cpu);
    assert_eq!(first.key(), second.key());
}

#[test]
fn network_group_keys_include_the_interface_name() {
    let stat = BuiltinStat::from_command("builtin:net:wlan0").expect("builtin stat");

    assert_eq!(
        stat.key(),
        BuiltinStatKey::Network {
            iface: Some("wlan0".to_string()),
        }
    );
}
