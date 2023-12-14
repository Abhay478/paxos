#![allow(dead_code)]
use std::{
    fmt::Debug,
    net::UdpSocket,
    sync::{Arc, Mutex},
};

use itertools::Itertools;
use serde_json::{from_slice, to_vec};

use crate::models::{Ballot, Message, Proposal};

type AcceptList = Arc<Mutex<Vec<Proposal>>>;
struct Acceptor {
    pub id: usize,
    pub ballot: Arc<Mutex<Ballot>>,
    pub accepted: AcceptList,
    pub sock: UdpSocket,
    buf: Vec<u8>,
}

impl Acceptor {
    pub fn new(id: usize, sock: UdpSocket) -> Acceptor {
        Acceptor {
            id,
            ballot: Arc::new(Mutex::new(Ballot::new(0, 0))),
            accepted: Arc::new(Mutex::new(Vec::new())),
            sock,
            buf: vec![],
        }
    }

    fn get_latest_accepts(&self) -> Vec<Proposal> {
        self.accepted
            .lock()
            .unwrap()
            .iter()
            .max_set()
            .into_iter()
            .map(|a| a.clone())
            .collect()
    }

    fn receive_prepare(&mut self, req: Message<()>) -> Message<()> {
        let mut u = self.ballot.lock().unwrap();
        if let Message::Phase1a(_num, ballot) = req {
            if ballot > *u {
                *u = ballot;
            }
            Message::Phase1b(ballot.leader_id, self.id, *u, self.get_latest_accepts())
        } else {
            unreachable!()
        }
    }

    fn receive_accept_request(&mut self, req: Message<()>) -> Message<()> {
        let u = self.ballot.lock().unwrap();
        if let Message::Phase2a(num, proposal) = req {
            if proposal.ballot == *u {
                self.accepted.lock().unwrap().push(proposal.clone());
            }
            Message::Phase2b(num, self.id, proposal)
        } else {
            unreachable!()
        }
    }

    fn handle(&mut self, req: Message<()>) -> Message<()> {
        match req {
            Message::Phase1a(_, _) => self.receive_prepare(req),
            Message::Phase2a(_, _) => self.receive_accept_request(req),
            _ => unreachable!(),
        }
    }
}

pub fn listen<T: Clone + Debug + serde::de::DeserializeOwned + serde::Serialize>(
    id: usize,
    sock: UdpSocket,
) {
    let mut q = Acceptor::new(id, sock);
    // let mut log = vec![];
    loop {
        let mut b = vec![];
        let (l, src) = q.sock.recv_from(&mut b).unwrap();

        let res = if let Ok(req) = from_slice(&b[..l]) {
            to_vec(&q.handle(req)).unwrap()
        } else {
            "Invalid message.".as_bytes().to_vec()
        };
        q.sock.send_to(&res, src).unwrap();
        // log.push(buf);
    }
}
