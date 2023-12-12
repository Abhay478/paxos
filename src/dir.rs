use std::net::UdpSocket;
use std::{fs::read_to_string, net::IpAddr};

use once_cell::sync::Lazy;
enum Actor {
    Acceptor,
    Proposer,
    Learner,
}

pub fn get_all_acceptors() -> Vec<(IpAddr, u16)> {
    let s = read_to_string("acceptors").unwrap();
    s.split("\n")
        .map(|s| {
            let s = s.split(":").collect::<Vec<_>>();
            (
                s[0].parse::<IpAddr>().unwrap(),
                s[1].parse::<u16>().unwrap(),
            )
        })
        .collect()
}

// pub fn get_acceptor_address(id: u32) -> (IpAddr, u16) {
//     // let f = File::open("acceptors").unwrap();
//     let s = read_to_string("acceptors").unwrap();
//     let s = s
//         .split("\n")
//         .nth(id as usize)
//         .unwrap()
//         .split(":")
//         .collect::<Vec<_>>();

//     (
//         s[0].parse::<IpAddr>().unwrap(),
//         s[1].parse::<u16>().unwrap(),
//     )
// }

pub fn broadcast_acceptor(msg: Vec<u8>, sock: &UdpSocket) {
    // static s: Vec<(IpAddr, u16)> = Lazy::new(|| get_all_acceptors()).to_vec();
    let s = get_all_acceptors();
    let _ = s
        .iter()
        .map(|(ip, port)| sock.send_to(&msg, (*ip, *port)).unwrap())
        .collect::<Vec<_>>();
}
