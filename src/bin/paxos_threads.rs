//! Run with
//! ```sh
//! cargo r --bin paxos_threads
//! ```
//!
//! in the root directory of the project.

use std::{
    thread,
    time::{Duration, Instant},
};

use dc_project::{
    paxos::{
        acceptor,
        dir::{
            acceptor_init, client_init, get_all_replicas, leader_init, replica_init,
            ACCEPTOR_COUNT, LEADER_COUNT, REPLICA_COUNT,
        },
        leader, replica, Command, Message,
    },
    Params,
};
use message_io::network::Transport;
use rand::seq::SliceRandom;
use serde_json::to_vec;
// use serde_json::to_vec;

fn main() {
    let params = Params::new();
    let sock = client_init();
    let reps = get_all_replicas(sock.0.clone());

    let mut acc_handles = vec![];
    for i in 0..ACCEPTOR_COUNT {
        let sock = acceptor_init(i.into());
        acc_handles.push(thread::spawn(move || {
            acceptor::listen(i.into(), sock.1, sock.0);
        }));
    }
    thread::sleep(Duration::from_secs(1));

    let mut lea_handles = vec![];
    for i in 0..LEADER_COUNT {
        let sock = leader_init(i.into());
        lea_handles.push(thread::spawn(move || {
            leader::listen(i.into(), sock.0, sock.1);
        }));
    }
    thread::sleep(Duration::from_secs(1));
    let mut rep_handles = vec![];
    for i in 0..REPLICA_COUNT {
        let sock = replica_init(i.into());
        rep_handles.push(thread::spawn(move || {
            replica::listen(i.into(), sock.1, sock.0);
        }));
    }

    thread::sleep(Duration::from_secs(1));

    let u = rand::distributions::Uniform::from(0.0..1.0);

    let rep = reps.choose(&mut rand::thread_rng()).unwrap();
    let _ = sock.0.network().connect(Transport::FramedTcp, rep.addr());
    let init = Instant::now();
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

    for handle in rep_handles {
        let _ = handle.join();
    }
    // println!("Here.");
    let done = init.elapsed();
    dbg!(done);

    // for handle in acc_handles {
    //     handle.join().unwrap();
    // }

    // for handle in lea_handles {
    //     handle.join().unwrap();
    // }
}

// fn main() {
//     // todo!()
//     let params = Params::new();
//     let reps = get_all_replicas();

// }
