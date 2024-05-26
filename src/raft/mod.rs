#![allow(dead_code)]
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

// use crate::paxos::Command;

// use self::server::{Campaign, Replicate};

pub mod dir;
pub mod server;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Command {
    pub client: SocketAddr,
    pub op_id: usize,
    pub op: String, // Small
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Log {
    term: usize,
    command: Option<Command>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Reply {
    pub from: usize,
    pub success: bool,
    pub term: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    Request(Command),
    Response(Command),
    Heartbeat(Replicate),
    Campaign(Campaign),
    ServerReply(Reply),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ServerState {
    Follower,
    Candidate(usize), // Contains number of votes
    Leader,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    term: usize,
    leader_id: usize,
    prev_log_index: usize,
    prev_log_term: usize,
    leader_commit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replicate {
    hb: Heartbeat,
    entries: Vec<(usize, Log)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Campaign {
    term: usize,
    candidate_id: usize,
    last_log_index: usize,
    last_log_term: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Timer {
    /// To send heartbeats. Contains prev_log_index
    Heartbeat,
    /// To initiate election
    Election,
}
