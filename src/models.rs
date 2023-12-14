use std::fmt::Debug;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ballot {
    pub num: usize,
    pub leader_id: usize,
}

impl Ballot {
    pub fn new(num: usize, leader_id: usize) -> Ballot {
        Ballot { num, leader_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Command {
    pub client_id: usize,
    pub op_id: usize,
    pub op: [u8; 16],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ClientRequest<T> {
    pub client_id: usize,
    pub op_id: usize,
    pub op: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub slot: usize,
    pub ballot: Ballot,
    pub command: Command,
}

impl PartialEq for Proposal {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.ballot == other.ballot
    }
}

impl Eq for Proposal {}

impl PartialOrd for Proposal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.slot == other.slot {
            self.ballot.partial_cmp(&other.ballot)
        } else {
            None
        }
    }
}

impl Ord for Proposal {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if let Some(c) = self.partial_cmp(other) {
            c
        } else {
            std::cmp::Ordering::Equal
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message<T: Clone + Debug + serde::Serialize> {
    // client <-> replica
    Request(ClientRequest<T>),
    Response(usize, String, Result<T, ()>),

    // replica <-> leader
    Propose(usize, Command),
    Decision(usize, Command),

    // leader <-> acceptor
    Phase1a(usize, Ballot),                       // acceptor id
    Phase1b(usize, usize, Ballot, Vec<Proposal>), // leader id, acceptor id,
    Phase2a(usize, Proposal),                     // leader id
    Phase2b(usize, usize, Proposal),              // leader id, acceptor id
}
