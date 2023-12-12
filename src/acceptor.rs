#![allow(dead_code)]
use std::{
    fmt::Debug,
    net::UdpSocket,
    sync::{Arc, Mutex},
};

use itertools::Itertools;
use serde_json::{from_slice, to_vec};

use crate::models::{Ballot, Message, Proposal};

type AcceptList<T> = Arc<Mutex<Vec<Proposal<T>>>>;
struct Acceptor<T: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned> {
    pub id: u32,
    pub ballot: Arc<Mutex<Ballot>>,
    pub accepted: AcceptList<T>,
    pub sock: UdpSocket,
    buf: Vec<u8>,
}

impl<T: Clone + Debug + serde::de::DeserializeOwned + serde::Serialize> Acceptor<T> {
    pub fn new(id: u32, sock: UdpSocket) -> Acceptor<T> {
        Acceptor {
            id,
            ballot: Arc::new(Mutex::new(Ballot::new(0, 0))),
            accepted: Arc::new(Mutex::new(Vec::new())),
            sock,
            buf: vec![],
        }
    }

    fn get_latest_accepts(&self) -> Vec<Proposal<T>> {
        self.accepted
            .lock()
            .unwrap()
            .iter()
            .max_set()
            .into_iter()
            .map(|a| a.clone())
            .collect()
    }

    fn receive_prepare(&mut self, req: Message<T>) -> Message<T> {
        let mut u = self.ballot.lock().unwrap();
        if let Message::Phase1a(_num, ballot, _) = req {
            if ballot > *u {
                *u = ballot;
            }
            Message::Phase1b(ballot.leader_id, self.id, *u, self.get_latest_accepts())
        } else {
            unreachable!()
        }
    }

    fn receive_accept_request(&mut self, req: Message<T>) -> Message<T> {
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

    fn handle(&mut self, req: Message<T>) -> Message<T> {
        match req {
            Message::Phase1a(_, _, _) => self.receive_prepare(req),
            Message::Phase2a(_, _) => self.receive_accept_request(req),
            _ => unreachable!(),
        }
    }
}

pub fn listen<T: Clone + Debug + serde::de::DeserializeOwned + serde::Serialize>(
    id: u32,
    sock: UdpSocket,
) {
    let mut q = Acceptor::<T>::new(id, sock);
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
