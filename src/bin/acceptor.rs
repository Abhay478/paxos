//! Code for acceptor.
//!
//!

use dc_project::{paxos::{acceptor, dir::acceptor_init}, NodeId};
use uuid::Uuid;
use std::env;

fn main() {
    // let id = env::args().nth(1).unwrap().parse::<usize>().unwrap();
    let id = *Uuid::new_v4().as_bytes();
    println!("Acceptor {:?}", id);

    let sock = acceptor_init(id);
    acceptor::listen(NodeId {id}, sock.1, sock.0);
}
