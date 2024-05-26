//! Code for replica
//!
//!

use dc_project::paxos::{dir::replica_init, replica};
use std::env;

fn main() {
    let id = env::args().nth(1).unwrap().parse::<usize>().unwrap();
    println!("Replica {}", id);

    let sock = replica_init(id);
    replica::listen(id, sock.1, sock.0);
}
