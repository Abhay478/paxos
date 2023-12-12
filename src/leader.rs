use std::{
    collections::HashMap,
    fmt::Debug,
    marker::PhantomData,
    net::{IpAddr, UdpSocket},
    sync::{Arc, Mutex},
};

use itertools::Itertools;
use serde_json::to_vec;
use std::hash::Hash;

use crate::{
    dir::get_all_acceptors,
    models::{Ballot, Command, Message, Proposal},
};

// TODO: Encode acceptor knowledge
// Done: Gave up and made a file.

struct Scout<T: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned> {
    wait: Vec<u32>,
    ballot: Ballot,
    pvalues: HashMap<u32, (Ballot, Command<T>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
// #[derive(Eq, Hash, PartialEq)]
struct CommId<
    T: Clone + Debug + PartialEq + Eq + Hash + serde::Serialize + serde::de::DeserializeOwned,
> {
    slot: u32,
    command: Command<T>,
}

struct CommState<T: Clone + Debug + serde::Serialize + serde::de::DeserializeOwned> {
    wait: Vec<u32>,
    ballot_num: u32,
    pd: PhantomData<T>,
}

struct Leader<T: Clone + Debug + Eq + Hash + serde::Serialize + serde::de::DeserializeOwned> {
    id: u32,
    active: bool,
    ballot_num: u32,
    proposals: Arc<Mutex<HashMap<u32, Command<T>>>>,
    scout: Option<Scout<T>>,
    sock: UdpSocket,
    acceptors: Vec<(IpAddr, u16)>,
    commanders: HashMap<CommId<T>, CommState<T>>,
}

impl<T: Clone + Debug + Eq + Hash + serde::Serialize + serde::de::DeserializeOwned> Leader<T> {
    pub fn new(id: u32, sock: UdpSocket) -> Leader<T> {
        Leader {
            id,
            active: false,
            ballot_num: 0,
            proposals: Arc::new(Mutex::new(HashMap::new())),
            scout: None,
            acceptors: get_all_acceptors(),
            sock,
            commanders: HashMap::new(),
        }
    }

    fn start_scout(&mut self) {
        self.active = true;
        self.ballot_num += 1;
        self.scout = Some(Scout {
            wait: vec![],
            ballot: Ballot::new(self.ballot_num, self.id),
            pvalues: HashMap::new(),
        });
        let msg = Message::Phase1a(
            self.id,
            Ballot {
                num: self.ballot_num,
                leader_id: self.id,
            },
            PhantomData::<T>::default(),
        );
        let buf = to_vec(&msg).unwrap();
        // broadcast_acceptor(buf, &self.sock);
        let _ = self
            .acceptors
            .iter()
            .map(|(ip, port)| self.sock.send_to(&buf, (*ip, *port)).unwrap())
            .collect::<Vec<_>>();
    }

    fn handle_propose(&mut self, msg: Message<T>) {
        if let Message::Propose(slot, command) = msg {
            {
                let mut proposals = self.proposals.lock().unwrap();
                if proposals.keys().contains(&slot) {
                    return;
                }
                proposals.insert(slot, command.clone());
            }
            if self.active {
                self.start_commander(slot, command);
            }
        }
    }

    fn start_commander(&mut self, slot: u32, command: Command<T>) {
        let cmdr = CommId {
            slot,
            command: command.clone(),
        };
        let state = CommState {
            wait: (0..(self.acceptors.len() as u32)).collect_vec(),
            ballot_num: self.ballot_num,
            pd: PhantomData::<T>::default(),
        };

        self.commanders.insert(cmdr, state);
        self.acceptors.iter().for_each(|(ip, port)| {
            let msg = Message::Phase2a(
                self.id,
                Proposal {
                    slot,
                    ballot: Ballot {
                        num: self.ballot_num,
                        leader_id: self.id,
                    },
                    command: command.clone(),
                },
            );
            let buf = to_vec(&msg).unwrap();
            let _ = self.sock.send_to(&buf, (*ip, *port)).unwrap();
        });
    }

    fn handle_p1b(&self, msg: Message<T>) {
        if self.scout.is_none() {
            return;
        }

        if let Message::Phase1b(acceptor_id, leader_id, ballot, pvalues) = msg {
            if ballot != self.scout.as_ref().unwrap().ballot {
                return;
            }
        }
    }
}
