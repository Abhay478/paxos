#![allow(dead_code)]

use itertools::Itertools;
use message_io::{
    network::NetEvent,
    node::{NodeHandler, NodeListener},
};
use serde_json::{from_slice, to_vec};
use sqlite::Connection;

use crate::{paxos::{Ballot, Message, Proposal}, NodeId};

// type AcceptList = Arc<Mutex<Vec<Proposal>>>;

/// Acceptor struct.
struct Acceptor {
    /// Used to be just a lil number. Unique among all acceptors.
    /// Now uuid.
    pub id: NodeId,
    /// The current ballot number of the acceptor. Important thing.
    pub ballot: Option<Ballot>,

    /// All the stuff so far.
    pub accepted: Vec<Proposal>,

    /// This is us.
    // pub sock: UdpSocket,
    // pub listener: NodeListener<()>,
    pub handler: NodeHandler<()>,

    db: Connection,
}

impl Acceptor {
    pub fn new(id: NodeId, handler: NodeHandler<()>) -> Acceptor {
        Acceptor {
            id,
            ballot: None,
            accepted: vec![],
            // listener,
            handler,
            db: Connection::open("paxos.db").unwrap(),
        }
    }

    pub fn with_conn(id: NodeId, handler: NodeHandler<()>, db: Connection) -> Acceptor {
        Acceptor {
            id,
            ballot: None,
            accepted: vec![],
            // listener,
            handler,
            db,
        }
    }

    fn get_latest_accepts(&self) -> Vec<Proposal> {
        self.accepted
            .iter()
            .max_set()
            .into_iter()
            .cloned()
            .collect()
    }

    /// Promise
    fn receive_p1(&mut self, ballot: Ballot) -> Message {
        // Just do it.
        if self.ballot.is_none() || ballot > self.ballot.unwrap() {
            self.ballot = Some(ballot);
        }

        // Send that damnation message.
        Message::Phase1b(
            ballot.leader_id,
            self.id,
            self.ballot.unwrap(),
            self.get_latest_accepts(),
        )
    }

    /// Accept
    fn receive_p2(&mut self, leader_id: NodeId, proposal: Proposal) -> Message {
        if Some(proposal.ballot) == self.ballot {
            self.accepted.push(proposal.clone());
        }
        Message::Phase2b(leader_id, self.id, proposal.ballot)
    }

    /// Mux
    fn handle(&mut self, req: Message) -> Message {
        // dbg!(&req);
        match req {
            Message::Phase1a(_num, ballot) => self.receive_p1(ballot),
            Message::Phase2a(lid, prop) => self.receive_p2(lid, prop),
            _ => unreachable!(),
        }
    }
}

/// This is the main loop for the acceptor.
/// Acceptors are pretty dumb, so there's not much going on here.
pub fn listen(id: NodeId, listener: NodeListener<()>, handler: NodeHandler<()>) {
    let mut q = Acceptor::new(id, handler);
    // println!("Inited acceptor {id}.");

    let _ = listener.for_each_async(move |event| match event.network() {
        NetEvent::Message(endpoint, buf) => {
            let res = if let Ok(req) = from_slice(&buf) {
                to_vec(&q.handle(req)).unwrap()
            } else {
                "Invalid message.".as_bytes().to_vec()
            };
            q.handler.network().send(endpoint, &res);
        }
        NetEvent::Connected(_ep, _) => {
            // println!("Acceptor {id} Connected to {ep}.");
        }
        NetEvent::Accepted(_ep, _) => {
            // println!("Acceptor {id} Accepted {ep}.");
        }
        NetEvent::Disconnected(_ep) => {
            // println!("Acceptor {id} Disconnected from {ep}.");
        } // _ => {}
    });
}
