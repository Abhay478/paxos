//! Code for client.
//!
//! Thinking each client produces a random value at random intervals and sends it to the replica.
//!
//! Which replica? Designated replica, random replica, or all replicas? RANDOM REPLICA.

use std::{env, net::SocketAddr};

use dc_project::{
    raft::{
        dir::{RAFT_COUNT, RAFT_PORT},
        Command, Message,
    },
    Params, LOOPBACK,
};
use message_io::{network::Transport, node};
use rand::{
    distributions::{Distribution, Uniform},
    thread_rng,
};
use serde_json::to_vec;

/// ```sh
/// cargo run --bin raft_client -- (client_id)
/// ```
fn main() {
    let params = Params::new();
    let sock = node::split::<()>();
    let client_id = env::args().nth(1).unwrap().parse::<usize>().unwrap();
    let addr = SocketAddr::from((LOOPBACK, 10000 + client_id as u16));
    let _ = sock.0.network().listen(Transport::Udp, addr).unwrap();

    let u = Uniform::from(0.0..1.0);
    let v = Uniform::from(0..RAFT_COUNT);
    let rep_idx = v.sample(&mut thread_rng());
    let rep = sock
        .0
        .network()
        .connect(
            Transport::Udp,
            SocketAddr::from((LOOPBACK, RAFT_PORT + rep_idx as u16)),
        )
        .unwrap()
        .0;

    println!("Sending to {:?}", rep);

    // let u = rand::distributions::Uniform::from(0.0..1.0);
    for i in 0..params.k {
        let val = rand::random::<u64>();
        let msg = Message::Request(Command {
            client: addr,
            op_id: i,
            op: val.to_string(),
        });
        dbg!(&msg);
        sock.0.network().send(rep, &to_vec(&msg).unwrap());
        params.sleep(u, &mut rand::thread_rng());
    }
    println!("Done.");
}
