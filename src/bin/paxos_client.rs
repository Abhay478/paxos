//! Code for client.
//!
//! Thinking each client produces a random value at random intervals and sends it to the replica.
//!
//! Which replica? Designated replica, random replica, or all replicas? RANDOM REPLICA.

use std::env;

use dc_project::{
    paxos::{
        dir::{client_init, get_all_replicas},
        Command, Message,
    },
    Params,
};
use rand::seq::SliceRandom;
use serde_json::to_vec;

/// ```sh
/// cargo run --bin paxos_client -- (client_id)
/// ```
///
/// Right now only written for Paxos.
fn main() {
    let params = Params::new();
    let (handler, _listener) = client_init();
    let reps = get_all_replicas(handler.clone());
    let rep = reps.choose(&mut rand::thread_rng()).unwrap();
    println!("Sending to {:?}", rep);
    let client_id = env::args().nth(1).unwrap().parse::<usize>().unwrap();
    let u = rand::distributions::Uniform::from(0.0..1.0);
    for i in 0..params.k {
        let val = rand::random::<u64>();
        let msg = Message::Request(Command {
            client_id,
            op_id: i,
            op: val.to_string(),
        });
        dbg!(&msg);
        handler.network().send(*rep, &to_vec(&msg).unwrap());
        params.sleep(u, &mut rand::thread_rng());
    }
    println!("Done.");
}
