use std::net::IpAddr;

use super::super::is_public_ip;

#[test]
fn remote_ip_policy_accepts_public_addresses_only() {
    for address in ["8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"] {
        let address = address.parse::<IpAddr>().expect("public address");
        assert!(is_public_ip(address), "{address}");
    }
    for address in [
        "127.0.0.1",
        "169.254.169.254",
        "10.0.0.1",
        "100.64.0.1",
        "192.0.2.1",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "::1",
        "fc00::1",
        "fe80::1",
        "2001:db8::1",
        "3fff::1",
        "::ffff:127.0.0.1",
        "2002:7f00:1::",
    ] {
        let address = address.parse::<IpAddr>().expect("non-public address");
        assert!(!is_public_ip(address), "{address}");
    }
}

#[test]
fn remote_ipv4_policy_covers_every_blocked_range_and_public_boundary() {
    let blocked = [
        "0.1.2.3",
        "10.1.2.3",
        "100.64.0.1",
        "100.127.255.254",
        "127.1.2.3",
        "169.254.1.1",
        "172.16.0.1",
        "172.31.255.254",
        "192.0.0.1",
        "192.0.2.1",
        "192.88.99.1",
        "192.168.1.1",
        "198.18.0.1",
        "198.19.255.254",
        "198.51.100.1",
        "203.0.113.1",
        "224.0.0.1",
        "255.255.255.255",
    ];
    for address in blocked {
        let address = address.parse::<IpAddr>().expect("valid blocked address");
        assert!(!is_public_ip(address), "{address}");
    }

    // Cross-field values catch accidental broadening of multi-octet range checks
    let public = [
        "8.0.0.1",
        "8.0.2.1",
        "8.18.0.1",
        "8.51.100.1",
        "8.64.0.1",
        "8.88.99.1",
        "8.168.0.1",
        "8.254.0.1",
        "100.63.255.254",
        "100.128.0.1",
        "126.255.255.254",
        "128.0.0.1",
        "169.253.1.1",
        "169.255.1.1",
        "172.15.0.1",
        "172.32.0.1",
        "191.255.255.254",
        "192.0.1.1",
        "192.0.3.1",
        "192.1.0.1",
        "192.1.2.1",
        "192.1.99.1",
        "192.87.99.1",
        "192.88.98.1",
        "192.167.1.1",
        "192.169.1.1",
        "197.255.255.254",
        "198.17.1.1",
        "198.20.1.1",
        "198.50.100.1",
        "198.51.99.1",
        "198.52.100.1",
        "202.255.255.254",
        "203.0.112.1",
        "203.0.114.1",
        "203.1.113.1",
        "223.255.255.254",
    ];
    for address in public {
        let address = address.parse::<IpAddr>().expect("valid public address");
        assert!(is_public_ip(address), "{address}");
    }
}

#[test]
fn remote_ipv6_policy_covers_mapped_6to4_and_unicast_boundaries() {
    let blocked = [
        "1fff::1",
        "4000::1",
        "2001:1::1",
        "2001:1ff::1",
        "2001:db8::1",
        "3fff::1",
        "3fff:fff::1",
        "::fffe:808:808",
        "::ffff:10.0.0.1",
        "::ffff:127.0.0.1",
        "2002:0a00:0001::1",
        "2002:7f00:0001::1",
        "2002:c000:0201::1",
    ];
    for address in blocked {
        let address = address.parse::<IpAddr>().expect("valid blocked address");
        assert!(!is_public_ip(address), "{address}");
    }

    let public = [
        "2000::1",
        "2001:200::1",
        "2001:db7::1",
        "2001:db9::1",
        "2002:0808:0808::1",
        "2606:100::1",
        "2fff:ffff::1",
        "3000::1",
        "3ffe:ffff::1",
        "3fff:1000::1",
        "::ffff:8.8.8.8",
        "2606:4700:4700::ffff:7f00:1",
    ];
    for address in public {
        let address = address.parse::<IpAddr>().expect("valid public address");
        assert!(is_public_ip(address), "{address}");
    }
}
