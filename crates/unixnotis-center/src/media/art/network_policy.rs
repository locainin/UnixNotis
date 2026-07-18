//! Remote artwork URL and destination-address policy

use std::net::IpAddr;

use url::Url;

pub fn remote_https_url_allowed(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.port_or_known_default() == Some(443)
}

pub fn is_public_ip(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(addr) => ipv4_is_public(addr.octets()),
        IpAddr::V6(addr) => ipv6_is_public(addr.segments()),
    }
}

fn ipv4_is_public([first, second, third, _fourth]: [u8; 4]) -> bool {
    // Reject non-routable, local, documentation, benchmark, multicast, and reserved ranges
    !(first == 0
        || first == 10
        || first == 127
        || (first == 100 && (64..=127).contains(&second))
        || (first == 169 && second == 254)
        || (first == 172 && (16..=31).contains(&second))
        || (first == 192 && second == 0 && third == 0)
        || (first == 192 && second == 0 && third == 2)
        || (first == 192 && second == 88 && third == 99)
        || (first == 192 && second == 168)
        || (first == 198 && matches!(second, 18 | 19))
        || (first == 198 && second == 51 && third == 100)
        || (first == 203 && second == 0 && third == 113)
        || first >= 224)
}

fn ipv6_is_public(segments: [u16; 8]) -> bool {
    // Mapped addresses inherit the IPv4 destination policy
    if segments[..5] == [0, 0, 0, 0, 0] && segments[5] == 0xffff {
        let high = segments[6].to_be_bytes();
        let low = segments[7].to_be_bytes();
        return ipv4_is_public([high[0], high[1], low[0], low[1]]);
    }

    // Current globally routed unicast space is 2000::/3
    if !(0x2000..=0x3fff).contains(&segments[0]) {
        return false;
    }
    // IETF assignments, documentation blocks, and the expanded documentation prefix stay local
    if (segments[0] == 0x2001 && (segments[1] <= 0x01ff || segments[1] == 0x0db8))
        || (segments[0] == 0x3fff && segments[1] <= 0x0fff)
    {
        return false;
    }
    if segments[0] == 0x2002 {
        // 6to4 embeds its eventual IPv4 destination in the next two segments
        let high = segments[1].to_be_bytes();
        let low = segments[2].to_be_bytes();
        return ipv4_is_public([high[0], high[1], low[0], low[1]]);
    }
    true
}
