//! Run with
//! ```sh
//! cargo r --bin raft_processes
//! ```
//!
//! in the root directory of the project.

use std::net::SocketAddr;
use std::time::Duration;
use std::{process, thread};

use dc_project::raft::dir::{raft_init, RAFT_COUNT, RAFT_PORT};
use dc_project::raft::{server, Command, Message};
use dc_project::{Params, LOOPBACK};
use message_io::network::Transport;
use message_io::node;
use rand::distributions::{Distribution, Uniform};
use rand::thread_rng;
use serde_json::to_vec;

fn main() {
    let params = Params::new();
    let sock = node::split::<()>();
    let _ = sock
        .0
        .network()
        .listen(Transport::Udp, SocketAddr::from((LOOPBACK, 10000)))
        .unwrap();
    // let handles = raft_init(); // Server threads spawn
    let mut handles = vec![];
    for i in 0..RAFT_COUNT {
        let mut cargo = process::Command::new("cargo");
        handles.push(
            cargo
                .args(["r", "--bin", "raft", "--", &i.to_string()])
                .spawn()
                .unwrap(),
        );
    }
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

    thread::sleep(Duration::from_secs(3));

    for i in 0..params.k {
        let val = rand::random::<u64>();
        let msg = Message::Request(Command {
            client: SocketAddr::from((LOOPBACK, 10000)),
            op_id: i,
            op: val.to_string(),
        });
        sock.0.network().send(rep, &to_vec(&msg).unwrap());
        params.sleep(u, &mut rand::thread_rng());
    }

    for h in handles.iter_mut() {
        h.wait().unwrap();
    }
}
