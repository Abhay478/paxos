//! Code for paxos leader
//!

use std::env;

use dc_project::paxos::{dir::leader_init, leader};

fn main() {
    let id = env::args().nth(1).unwrap().parse::<usize>().unwrap();
    println!("Leader {}", id);

    let sock = leader_init(id);
    leader::listen(id, sock.0, sock.1);
}
