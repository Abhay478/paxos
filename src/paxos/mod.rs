//! Achieve consensus on a sequence of Strings using the Paxos algorithm.

pub mod acceptor;
pub mod dir;
pub mod leader;
pub mod replica;

use std::{fmt::Debug, net::IpAddr};

use serde_derive::{Deserialize, Serialize};

use crate::{Identity, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ballot {
    pub num: usize,
    pub leader_id: NodeId,
}

impl Ballot {
    pub fn new(num: usize, leader_id: NodeId) -> Ballot {
        Ballot { num, leader_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Command {
    pub client_id: usize,
    pub op_id: usize,
    pub op: String, // Small
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

/// TODO: Replace usize with NodeID wherever necessary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // client <-> replica
    Request(Command),
    Response(usize, String, Result<String, String>), // Does the content matter?

    // replica <-> leader
    Propose(usize, Command),
    Decision(usize, Command),

    // leader <-> acceptor
    Phase1a(NodeId, Ballot),                       // acceptor id
    Phase1b(NodeId, NodeId, Ballot, Vec<Proposal>), // leader id, acceptor id,
    Phase2a(NodeId, Proposal),                     // leader id
    Phase2b(NodeId, NodeId, Ballot),                // leader id, acceptor id

    /// Special
    /// Each time a new node joins the cluster, it sends this message to all other nodes.
    /// Its credentials are added to the db, and the other node responds with the same message.
    /// 
    /// The bool avoids an infinite loop.
    Identify(Identity, IpAddr, NodeId, bool), // I am me.
}
