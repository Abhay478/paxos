#![allow(dead_code)]
use std::{
    net::SocketAddr,
    thread::{self, JoinHandle},
};

use hashbrown::HashMap;
use message_io::{
    network::{Endpoint, Transport},
    node::NodeHandler,
};

use crate::LOOPBACK;

use super::server;

pub const RAFT_PORT: u16 = 9000;
pub const RAFT_COUNT: usize = 5;

pub fn get_peers<Y>(id: usize, handler: NodeHandler<Y>) -> HashMap<usize, Endpoint> {
    (0..RAFT_COUNT)
        .filter(|&i| i != id)
        .map(|i| {
            let out = handler
                .network()
                .connect(
                    Transport::Udp,
                    SocketAddr::from((LOOPBACK, RAFT_PORT + i as u16)),
                )
                .unwrap();
            // dbg!(&out);
            (i, out.0)
        })
        .collect()
}

/* pub fn get_all_servers(handler: &NodeHandler<()>) -> Vec<Endpoint> {
    (0..RAFT_COUNT)
        .map(|i| {
            let out = handler
                .network()
                .connect(
                    Transport::Udp,
                    SocketAddr::from((LOOPBACK, RAFT_PORT + i as u16)),
                )
                .unwrap();
            // dbg!(&out);
            out.0
        })
        .collect()
}
 */
pub fn raft_init() -> Vec<JoinHandle<()>> {
    let mut out = vec![];
    for i in 0..RAFT_COUNT {
        out.push(thread::spawn(move || {
            server::run(i, SocketAddr::from((LOOPBACK, RAFT_PORT + i as u16)));
        }));
    }

    out
}
