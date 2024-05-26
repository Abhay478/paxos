use std::{fmt::Display, fs::File, io::Read, net::SocketAddr, thread, time::Duration};

use message_io::node::NodeHandler;
use rand::{
    distributions::{Distribution, Uniform},
    rngs::ThreadRng,
};
use serde::{Deserialize, Serialize};
use sqlite::Connection;
use uuid::Uuid;

pub const LOOPBACK: [u8; 4] = [127, 0, 0, 1];

pub mod paxos;
pub mod raft;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Identity {
    /// Paxos trio.
    Acceptor,
    Leader,
    Replica,
    /// Raft server.
    Server,
}

impl Display for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Identity::Acceptor => write!(f, "Acceptor"),
            Identity::Leader => write!(f, "Leader"),
            Identity::Replica => write!(f, "Replica"),
            Identity::Server => write!(f, "Server"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Params {
    pub k: usize,
    l: f64,
}

impl Params {
    pub fn new() -> Self {
        let mut file = File::open("inp-params.txt").unwrap();
        let mut buf = String::new();
        file.read_to_string(&mut buf).unwrap();

        let q = buf
            .split_whitespace()
            .map(|x| x.parse::<f64>().unwrap())
            .collect::<Vec<f64>>();

        Self {
            k: q[0] as usize,
            l: q[1],
        }
    }

    pub fn get_delay(u: Uniform<f64>, rng: &mut ThreadRng, l: f64) -> Duration {
        let ts = -(u.sample(rng) as f64).ln() * l;
        Duration::from_millis(ts as u64)
    }

    pub fn sleep(&self, u: Uniform<f64>, rng: &mut ThreadRng) {
        thread::sleep(Self::get_delay(u, rng, self.l));
    }
}

/// Right now this is just a `usize`, but it can really be anything. The rest of the code is general enough.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default, Copy)]
pub struct ReplicaState {
    n: usize,
}

impl ReplicaState {
    pub fn triv(s: String) -> impl Fn(&ReplicaState) -> (ReplicaState, Result<String, String>) {
        move |q| (*q, Ok(s.clone()))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    pub id: [u8; 16], // Uuid.
}

impl NodeId {
    pub fn new() -> Self {
        Self {
            id: *Uuid::new_v4().as_bytes(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub id: NodeId,
    pub kind: Identity,
    pub addr: SocketAddr,
}

impl Entry {
    pub fn new(id: NodeId, kind: Identity, addr: SocketAddr) -> Self {
        Self {
            id,
            kind,
            addr,
        }
    }
}


/// TODO: Replace all handler-addr-db triples with an Aux.
pub struct Aux<T> {
    pub handler: NodeHandler<T>,
    pub addr: SocketAddr,
    pub db: Connection,
}