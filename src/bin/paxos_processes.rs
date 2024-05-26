//! Run with
//! ```sh
//! cargo r --bin paxos_processes
//! ```
//!
//! in the root directory of the project.

use std::{process::Stdio, thread, time::Duration};

use dc_project::{
    paxos::{
        dir::{client_init, get_all_replicas, ACCEPTOR_COUNT, LEADER_COUNT, REPLICA_COUNT},
        Command, Message,
    },
    Params,
};
use message_io::network::Transport;
use rand::seq::SliceRandom;
use serde_json::to_vec;
use std::process;
// use serde_json::to_vec;

fn main() {
    let params = Params::new();
    let sock = client_init();
    let reps = get_all_replicas(sock.0.clone());

    let mut acc_handles = vec![];

    for i in 0..ACCEPTOR_COUNT {
        let mut cargo = process::Command::new("cargo");
        acc_handles.push(
            cargo
                .args(["r", "--bin", "acceptor", "--", &i.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    thread::sleep(Duration::from_secs(1));

    let mut lea_handles = vec![];
    for i in 0..LEADER_COUNT {
        let mut cargo = process::Command::new("cargo");
        lea_handles.push(
            cargo
                .args(["r", "--bin", "leader", "--", &i.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    thread::sleep(Duration::from_secs(1));

    let mut rep_handles = vec![];
    for i in 0..REPLICA_COUNT {
        let mut cargo = process::Command::new("cargo");
        rep_handles.push(
            cargo
                .args(["r", "--bin", "replica", "--", &i.to_string()])
                .spawn()
                .unwrap(),
        );
    }

    thread::sleep(Duration::from_secs(1));

    let u = rand::distributions::Uniform::from(0.0..1.0);

    let rep = reps.choose(&mut rand::thread_rng()).unwrap();
    let _ = sock.0.network().connect(Transport::FramedTcp, rep.addr());
    for i in 0..params.k {
        let val = rand::random::<u64>();
        let msg = Message::Request(Command {
            client_id: 0,
            op_id: i,
            op: val.to_string(),
        });
        sock.0.network().send(*rep, &to_vec(&msg).unwrap());
        params.sleep(u, &mut rand::thread_rng());
    }

    for handle in acc_handles.iter_mut() {
        handle.wait().unwrap();
    }

    // for handle in lea_handles.iter_mut() {
    //     handle.wait().unwrap();
    // }

    // for handle in rep_handles.iter_mut() {
    //     handle.wait().unwrap();
    // }

    // for handle in lea_handles {
    //     handle.join().unwrap();
    // }

    // for handle in rep_handles {
    //     handle.join().unwrap();
    // }
}
