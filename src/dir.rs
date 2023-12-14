#![allow(dead_code)]
use std::{fs::read_to_string, net::SocketAddr};

enum Actor {
    Acceptor,
    Proposer,
    Learner,
}

fn read_file(filename: &str) -> Vec<SocketAddr> {
    let s = read_to_string(filename).unwrap();
    s.split("\n")
        .map(|s| s.parse::<SocketAddr>().unwrap())
        .collect()
}

pub fn get_all_acceptors() -> Vec<SocketAddr> {
    read_file("acceptors")
}

pub fn get_all_leaders() -> Vec<SocketAddr> {
    read_file("leaders")
}
