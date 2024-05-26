//! Code for server.
//!
//!

use dc_project::{
    raft::{dir::RAFT_PORT, server},
    LOOPBACK,
};
use std::{env, net::SocketAddr};

fn main() {
    let id = env::args().nth(1).unwrap().parse::<usize>().unwrap();
    println!("Server {}", id);

    server::run(id, SocketAddr::from((LOOPBACK, RAFT_PORT + id as u16)));
}
