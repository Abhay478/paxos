use std::{fmt::Debug, marker::PhantomData};

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Ballot {
    pub num: u32,
    pub leader_id: u32,
}

impl Ballot {
    pub fn new(num: u32, leader_id: u32) -> Ballot {
        Ballot { num, leader_id }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Command<T: Clone + Debug + serde::Serialize> {
    pub client_id: i32,
    pub op_id: i32,
    pub op: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal<T: Clone + Debug + serde::Serialize> {
    pub slot: u32,
    pub ballot: Ballot,
    pub command: Command<T>,
}

impl<T: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned> PartialEq for Proposal<T> {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot && self.ballot == other.ballot
    }
}

impl<T: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned> Eq for Proposal<T> {}

impl<T: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned> PartialOrd for Proposal<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self.slot == other.slot {
            self.ballot.partial_cmp(&other.ballot)
        } else {
            None
        }
    }
}

impl<T: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned> Ord for Proposal<T> {
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
    Request(Command<T>),
    Response(Command<T>, String),

    // replica <-> leader
    Propose(u32, Command<T>),
    Decision(u32, Command<T>),

    // leader <-> acceptor
    Phase1a(u32, Ballot, PhantomData<T>),        // acceptor id
    Phase1b(u32, u32, Ballot, Vec<Proposal<T>>), // leader id, acceptor id,
    Phase2a(u32, Proposal<T>),                   // leader id
    Phase2b(u32, u32, Proposal<T>),              // leader id, acceptor id
}
