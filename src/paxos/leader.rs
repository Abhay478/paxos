use std::{collections::HashMap, sync::Arc, thread, time::Duration};

use message_io::{
    network::{Endpoint, NetEvent},
    node::{NodeEvent, NodeHandler, NodeListener},
};

use serde_json::to_vec;
use sqlite::Connection;

use crate::{paxos::dir::commander_init, NodeId};

use super::{
    dir::{get_all_acceptors, get_all_replicas, scout_init},
    Ballot, Message, Proposal,
};

/// 'Return type' of a Scout or Commander thread.
/// Sent through a channel to the main thread.
#[derive(Debug, Clone)]
pub enum Agent {
    Committed,
    Adopted(Ballot, HashMap<usize, Vec<Proposal>>),
    Preempted(Ballot),
}

impl Agent {
    /// Call this in a separate thread
    pub fn init_commander(
        prop: Proposal,
        acceptors: Arc<Vec<Endpoint>>,
        replicas: Arc<Vec<Endpoint>>,
        // sock: Arc<UdpSocket>,
        handler: NodeHandler<()>,
        listener: NodeListener<()>,
        other_handler: NodeHandler<Agent>,
        // agent_tx: Arc<Sender<Agent>>,
        lid: NodeId,
    ) {
        // dbg!("Commander.");
        let mut waitfor = (*acceptors).clone();
        let msg = Message::Phase2a(lid, prop.clone());

        for acc in acceptors.iter() {
            // sock.send_to(&to_vec(&msg).unwrap(), acc).await.unwrap();
            handler.network().send(*acc, &to_vec(&msg).unwrap());
        }

        // loop {
        listener.for_each(move |event| {
            match event.network() {
                NetEvent::Message(endpoint, message) => {
                    let msg: Message = serde_json::from_slice(&message).unwrap();
                    // dbg!(&msg);
                    match msg {
                        Message::Phase2b(_back_lid, _acc_id, blt) => {
                            if blt == prop.ballot {
                                // Using retain coz remove wants the index.
                                waitfor.retain(|x| *x != endpoint);

                                if waitfor.len() < acceptors.len() / 2 {
                                    // Majority
                                    let rep_msg =
                                        Message::Decision(prop.slot, prop.command.clone());

                                    for rep in replicas.iter() {
                                        // sock.send_to(&to_vec(&rep_msg).unwrap(), rep).await.unwrap();
                                        other_handler
                                            .network()
                                            .send(*rep, &to_vec(&rep_msg).unwrap());
                                    }
                                    // agent_tx.send(Self::Committed).unwrap();
                                    other_handler.signals().send(Self::Committed);
                                    return;
                                }
                            } else {
                                // agent_tx.send(Self::Preempted(blt)).unwrap();
                                other_handler.signals().send(Self::Preempted(blt));
                                return;
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        });
        /* let mut buf = vec![0; 1024];
        let (len, addr) = sock.recv_from(&mut buf).await.unwrap();
        let msg: Message = serde_json::from_slice(&buf[..len]).unwrap();
        match msg {
            Message::Phase2b(_back_lid, _acc_id, blt) => {
                if blt == prop.ballot {
                    // Using retain coz remove wants the index.
                    waitfor.retain(|x| *x != addr);

                    if waitfor.len() < acceptors.len() / 2 {
                        // Majority
                        let rep_msg = Message::Decision(prop.slot, prop.command.clone());

                        for rep in replicas.iter() {
                            sock.send_to(&to_vec(&rep_msg).unwrap(), rep).await.unwrap();
                        }
                        agent_tx.send(Self::Committed).unwrap();
                        return;
                    }
                } else {
                    agent_tx.send(Self::Preempted(blt)).unwrap();
                    return;
                }
            }
            _ => {}
        }
         */
        // }
    }

    pub fn init_scout(
        lid: NodeId,
        mut ballot: Ballot,
        acceptors: Arc<Vec<Endpoint>>,
        listener: NodeListener<Ballot>,
        handler: NodeHandler<Ballot>,
        other_handler: NodeHandler<Agent>, // communicate with leader.
    ) {
        let mut waitfor = (*acceptors).clone();
        // loop {
        // let mut ballot;
        let msg = Message::Phase1a(lid, ballot);

        for acc in acceptors.iter() {
            // sock.send_to(&to_vec(&msg).unwrap(), acc).await.unwrap();
            handler.network().send(*acc, &to_vec(&msg).unwrap());
        }

        let mut pvals = HashMap::<usize, Vec<Proposal>>::new();
        let _ = listener.for_each_async(move |event| {
            match event {
                NodeEvent::Network(u) => match u {
                    NetEvent::Message(endpoint, message) => {
                        let msg: Message = serde_json::from_slice(&message).unwrap();
                        // dbg!(&msg);
                        match msg {
                            Message::Phase1b(_lid, _acc_id, blt, accepts) => {
                                if blt == ballot {
                                    // dbg!(&endpoint);
                                    waitfor.retain(|x| x.addr() != endpoint.addr());
                                    accepts.iter().for_each(|acc| {
                                        if let Some(p) = pvals.get_mut(&acc.slot) {
                                            p.push(acc.clone());
                                        } else {
                                            pvals.insert(acc.slot, vec![acc.clone()]);
                                        }
                                    });

                                    // dbg!(&waitfor, &pvals, &acceptors);

                                    if (waitfor.len() as f64) < acceptors.len() as f64 / 2.0 {
                                        // Majority
                                        // dbg!("Majority");
                                        other_handler
                                            .signals()
                                            .send(Self::Adopted(blt, pvals.clone()));
                                        return;
                                    }
                                } else {
                                    other_handler.signals().send(Self::Preempted(blt));
                                    return;
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => {}
                },
                NodeEvent::Signal(s) => {
                    ballot = s;
                    let msg = Message::Phase1a(lid, ballot);
                    for acc in acceptors.iter() {
                        // sock.send_to(&to_vec(&msg).unwrap(), acc).await.unwrap();
                        handler.network().send(*acc, &to_vec(&msg).unwrap());
                    }
                }
            }
        });
    }
}

/// Leader struct. Most of the action happens here.
pub struct Leader {
    /// Just a lil number. Unique among all leaders.
    id: NodeId,
    //// Set of all outstanding proposals.
    proposals: HashMap<usize, Proposal>,
    /// State of the scout.
    active: bool,
    /// Current ballot.
    ballot: Ballot,
    db: Connection,
}

impl Leader {
    pub fn new(id: NodeId) -> Self {
        Self {
            id,
            proposals: HashMap::new(),
            active: false,
            ballot: Ballot::new(0, id),
            db: Connection::open("paxos.db").unwrap(),
        }
    }

    pub fn with_conn(id: NodeId, db: Connection) -> Self {
        Self {
            id,
            proposals: HashMap::new(),
            active: false,
            ballot: Ballot::new(0, id),
            db,
        }
    }

    pub fn update(&mut self, pmax: HashMap<usize, Proposal>) {
        self.proposals.retain(|s, p| match pmax.get(&s) {
            Some(val) => val.command == p.command,
            None => true,
        });

        self.proposals.extend(pmax.into_iter());
    }
}

pub fn get_pmax(pvals: &HashMap<usize, Vec<Proposal>>) -> HashMap<usize, Proposal> {
    pvals
        .into_iter()
        .map(|(slot, prop)| {
            (
                *slot,
                prop.into_iter()
                    .max_by_key(|p| p.ballot.num)
                    .unwrap()
                    .clone(),
            )
        })
        .collect::<HashMap<usize, Proposal>>()
}

/// TODO: Add file read for lists.
pub fn listen(id: NodeId, handler: NodeHandler<Agent>, listener: NodeListener<Agent>) {
    let mut leader = Leader::new(id);
    let acceptors = Arc::new(get_all_acceptors(handler.clone(), &leader.db));
    thread::sleep(Duration::from_secs(2));
    let replicas = Arc::new(get_all_replicas(handler.clone(), &leader.db));

    // println!("Inited leader {}", id);

    let mut commanders = vec![];
    let new_acc = acceptors.clone();

    let (scout_h, scout_l) = scout_init(&acceptors);
    let oh = handler.clone();
    let sh = scout_h.clone();

    let _scout = thread::spawn(move || {
        Agent::init_scout(leader.id, leader.ballot, new_acc, scout_l, sh, oh);
    }); // Sus

    let _ = listener.for_each_async(move |event| {
        match event {
            NodeEvent::Signal(s) => {
                // dbg!(&s);
                match s {
                    Agent::Adopted(_blt, pvals) => {
                        // leader.ballot.num = blt.num + 1;
                        let pmax = get_pmax(&pvals);
                        leader.update(pmax);

                        // This is bad. Too many clones. That said, it is Arc, so maybe we can get away with it.
                        for (_s, p) in leader.proposals.iter() {
                            // let alt_sock = arc_sock.clone();
                            let new_acc = acceptors.clone();
                            let new_rep = replicas.clone();
                            // let new_tx = agent_tx.clone();
                            let q = p.clone();

                            let (h, l) = commander_init(&acceptors);
                            let oh = handler.clone();
                            commanders.push(thread::spawn(move || {
                                Agent::init_commander(q, new_acc, new_rep, h, l, oh, leader.id)
                            }));
                        }

                        leader.active = true;
                    }
                    Agent::Preempted(blt) => {
                        if blt > leader.ballot {
                            leader.active = false;
                            leader.ballot.num = blt.num + 1;
                            // Pseudocode restarts the thread here. We just update the ballot. Message passing cheaper than spawning.
                            scout_h.signals().send(leader.ballot);
                        }
                    }
                    Agent::Committed => {} // Not given. WTF.
                }
            }
            NodeEvent::Network(u) => match u {
                NetEvent::Message(_endpoint, buf) => {
                    let msg: Message = serde_json::from_slice(&buf).unwrap();
                    // dbg!(&msg);
                    match msg {
                        Message::Propose(slot, cmd) => {
                            // if let Some(_) = leader.proposals.get(&slot) {
                            //     // Proposal is lost here. Correctness check.
                            //     return;
                            // }

                            let prop = Proposal {
                                slot,
                                ballot: leader.ballot,
                                command: cmd,
                            };
                            leader.proposals.insert(slot, prop.clone());

                            let new_acc = acceptors.clone();
                            let new_rep = replicas.clone();
                            // let new_tx = agent_tx.clone();
                            let oh = handler.clone();
                            if leader.active {
                                let (h, l) = commander_init(&acceptors);
                                commanders.push(thread::spawn(move || {
                                    Agent::init_commander(
                                        prop, new_acc, new_rep, h, l, oh, leader.id,
                                    )
                                }))
                            }
                        }
                        /* Message::Terminate => {
                            return;
                        } */
                        _ => {}
                    }
                }
                NetEvent::Accepted(_ep, _) => {
                    // println!("Leader {id} Accepted {ep}.");
                }
                NetEvent::Connected(_ep, _) => {
                    // println!("Leader {id} Connected to {ep}.");
                }
                NetEvent::Disconnected(_ep) => {
                    // println!("Leader {id} Disconnected from {ep}.");
                } // _ => {}
            },
        }
    });
    // todo!()
}
