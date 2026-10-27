use std::net::SocketAddr;

pub fn address(dc_id: i32) -> Option<SocketAddr> {
    let ip = match dc_id {
        1 => [149, 154, 175, 53],
        2 => [149, 154, 167, 51],
        3 => [149, 154, 175, 100],
        4 => [149, 154, 167, 91],
        5 => [91, 108, 56, 130],
        _ => return None,
    };
    Some(SocketAddr::from((ip, 443)))
}
