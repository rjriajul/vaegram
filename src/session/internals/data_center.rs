use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use crate::raw::base;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Dc {
    pub id: i32,
    pub addr: SocketAddr,
    pub test: bool,
}

impl Dc {
    /// The id handshakes and transport headers carry: test DCs add 10000.
    pub fn wire_id(&self) -> i32 {
        if self.test { self.id + 10000 } else { self.id }
    }
}

pub fn address(dc_id: i32, test: bool, ipv6: bool) -> Option<SocketAddr> {
    let v4 = |a, b, c, d| IpAddr::V4(Ipv4Addr::new(a, b, c, d));
    let host = if test { 0xe } else { 0xa };
    let v6 = |a, b, c| IpAddr::V6(Ipv6Addr::new(0x2001, a, b, c, 0, 0, 0, host));
    let ip = match (test, ipv6, dc_id) {
        (false, false, 1) => v4(149, 154, 175, 53),
        (false, false, 2) => v4(149, 154, 167, 51),
        (false, false, 3) => v4(149, 154, 175, 100),
        (false, false, 4) => v4(149, 154, 167, 91),
        (false, false, 5) => v4(91, 108, 56, 130),
        (true, false, 1) => v4(149, 154, 175, 10),
        (true, false, 2) => v4(149, 154, 167, 40),
        (true, false, 3) => v4(149, 154, 175, 117),
        (_, true, 1) => v6(0xb28, 0xf23d, 0xf001),
        (_, true, 2) => v6(0x67c, 0x4e8, 0xf002),
        (_, true, 3) => v6(0xb28, 0xf23d, 0xf003),
        (false, true, 4) => v6(0x67c, 0x4e8, 0xf004),
        (false, true, 5) => v6(0xb28, 0xf23f, 0xf005),
        _ => return None,
    };
    Some(SocketAddr::new(ip, 443))
}

// CDN DCs stay in the main table: their ids are their own, and `Client::dc` finds them for redirects.
pub fn table(options: &[base::DcOption], ipv6: bool, media: bool) -> Vec<(i32, SocketAddr)> {
    let mut table = Vec::new();
    for base::DcOption::DcOption(o) in options {
        if o.media_only != media || o.tcpo_only || o.ipv6 != ipv6 || table.iter().any(|(id, _)| *id == o.id) {
            continue;
        }
        if let (Ok(ip), Ok(port)) = (o.ip_address.parse(), u16::try_from(o.port)) {
            table.push((o.id, SocketAddr::new(ip, port)));
        }
    }
    table
}

pub fn find(table: &[(i32, SocketAddr)], dc_id: i32, test: bool, ipv6: bool) -> Option<Dc> {
    let addr = match table.iter().find(|(id, _)| *id == dc_id) {
        Some(&(_, addr)) => addr,
        None => address(dc_id, test, ipv6)?,
    };
    Some(Dc { id: dc_id, addr, test })
}

#[cfg(test)]
mod tests {
    use super::{address, find, table};
    use crate::raw::{base, types};

    fn option(id: i32, ip: &str, ipv6: bool, media_only: bool) -> base::DcOption {
        base::DcOption::DcOption(Box::new(types::DcOption {
            ipv6,
            media_only,
            tcpo_only: false,
            cdn: false,
            r#static: false,
            this_port_only: false,
            id,
            ip_address: ip.into(),
            port: 443,
            secret: None,
        }))
    }

    #[test]
    fn picks_first_usable_option_per_dc() {
        let mut tcpo = option(3, "10.0.0.3", false, false);
        let base::DcOption::DcOption(o) = &mut tcpo;
        o.tcpo_only = true;
        let mut cdn = option(203, "10.0.2.3", false, false);
        let base::DcOption::DcOption(o) = &mut cdn;
        o.cdn = true;
        let options = [
            option(1, "10.0.0.9", false, true),
            option(1, "2001:db8::1", true, false),
            option(1, "10.0.0.1", false, false),
            option(1, "10.0.0.2", false, false),
            option(1, "10.0.0.8", false, true),
            option(2, "not an ip", false, false),
            tcpo,
            cdn,
        ];
        let v4 = table(&options, false, false);
        assert_eq!(v4, [(1, "10.0.0.1:443".parse().unwrap()), (203, "10.0.2.3:443".parse().unwrap())]);
        assert_eq!(table(&options, true, false), [(1, "[2001:db8::1]:443".parse().unwrap())]);
        assert_eq!(table(&options, false, true), [(1, "10.0.0.9:443".parse().unwrap())]);

        assert_eq!(find(&v4, 1, false, false).unwrap().addr, "10.0.0.1:443".parse().unwrap());
        assert_eq!(find(&v4, 2, false, false).unwrap().addr, address(2, false, false).unwrap());
        assert!(find(&v4, 9, false, false).is_none());
    }

    #[test]
    fn knows_built_in_test_and_ipv6_addresses() {
        assert_eq!(address(2, true, false), Some("149.154.167.40:443".parse().unwrap()));
        assert_eq!(address(2, false, true), Some("[2001:67c:4e8:f002::a]:443".parse().unwrap()));
        assert_eq!(address(3, true, true), Some("[2001:b28:f23d:f003::e]:443".parse().unwrap()));
        assert_eq!(address(4, true, false), None);
    }
}
