#![allow(dead_code)]
use crate::{Params, ReplicaState};

use self::dir::{get_all_leaders, remember_node};
use hashbrown::HashMap;
use local_ip_address::local_ip;
use message_io::{
    network::{Endpoint, NetEvent},
    node::{NodeHandler, NodeListener},
};
use serde_json::{from_slice, to_vec};
use sqlite::Connection; // Might have to change this to bincode or a custom impl.
use std::collections::BTreeMap;

use super::*;

const WINDOW: usize = 32;

/// This can be something as simple as
/// ```
/// |q: ReplicaState| (q, Ok(""))
/// ```
/// in which case we'd be storing constants and not operations.
///
/// The triv() function does just that.
pub struct Op {
    /// Change to SocketAddr?
    client_id: usize,
    /// Sequence number.
    op_id: usize,
    /// The operation to be performed.
    op: Box<dyn Fn(&ReplicaState) -> (ReplicaState, Result<String, String>) + Send + Sync>,
}

/// Node struct.
pub struct Replica {
    /// Just a lil number. Unique among all replicas.
    id: NodeId,
    /// Eh, just some state.
    state: ReplicaState,
    /// Things for the algorithm.
    slot_in: usize,
    slot_out: usize,
    /// Outstanding requests from clients
    requests: Vec<Command>,
    /// Outstaning proposals that have been sent out, but not decided upon.
    proposals: BTreeMap<usize, Command>,
    /// These are the done deals.
    decisions: HashMap<usize, Command>,

    /// These are the guys you gotta talk to.
    // leaders: Vec<Endpoint>,

    /// This is us.
    // sock: UdpSocket,
    // listener: NodeListener<()>,
    handler: NodeHandler<()>,

    /// These are those icky clients that keep bothering us.
    clients: HashMap<usize, Endpoint>,

    db: Connection,
}

impl Replica {
    pub fn new(id: NodeId, handler: NodeHandler<()>) -> Self {
        Self {
            id,
            state: ReplicaState::default(),
            slot_in: 0,
            slot_out: 0,
            requests: vec![],
            proposals: BTreeMap::new(),
            decisions: HashMap::new(),
            handler,
            clients: HashMap::new(),
            db: Connection::open("paxos.db").unwrap(),
        }
    }

    pub fn with_conn(id: NodeId, handler: NodeHandler<()>, db: Connection) -> Self {
        Self {
            id,
            state: ReplicaState::default(),
            slot_in: 0,
            slot_out: 0,
            requests: vec![],
            proposals: BTreeMap::new(),
            decisions: HashMap::new(),
            handler,
            clients: HashMap::new(),
            db,
        }
    }

    /// Self explanatory name.
    ///
    /// Each proposal is removed from `requests`, topped off with a slot, and sent to all leaders.
    /// This is done for multiple requests, each getting a different slot.
    fn propose(&mut self, leaders: &Vec<Endpoint>) {
        while self.slot_in < self.slot_out + WINDOW && !self.requests.is_empty() {
            if self.decisions.get(&self.slot_in).is_none() {
                let c = self.requests.pop().unwrap(); // do this
                self.proposals.insert(self.slot_in, c.clone()); // and then do that
                let msg = Message::Propose(self.slot_in, c); // And the this.

                // self.proposals[&self.slot_in] = c;

                let buf = to_vec(&msg).unwrap();

                // Now send the bloody thing
                leaders.iter().for_each(|addr| {
                    self.handler.network().send(*addr, &buf);
                });
            }
            self.slot_in += 1;
        }
        // println!("PROPOSE");
    }

    /// Simple pipeline.
    /// Gets thing from leader, sends thing to client.
    /// Shimpul.
    fn perform(&mut self, op: Command) {
        /*
            NOTE:
            - Pseudocode has this particular if block so as to avoid duplicate executions in case one command is decided at multiple slots.
            - Since all replicas have the same sequence of decisions, this is merely an optimisation.
            - We contend that a command may mutate some external state, and hence is not idempotent.
            - Thus, this block has been commented out.
            - We *are* keeping this, just in case.
        */

        // if self.decisions.contains(&Some(op)) {
        //     self.slot_out += 1;
        //     return;
        // }

        // dbg!(&self.clients, &op);
        let addr = self.clients.get(&op.client_id);
        let (state, res) = ReplicaState::triv(op.op)(&self.state);
        // For some reason, this should be atomic, but since we're not using threads, it's fine.
        {
            // let _un = self.lock.lock().unwrap();
            self.state = state;
            self.slot_out += 1;
        }
        // dbg!("PERFORM");

        if let Some(addr) = addr {
            // TODO: Change the contents of Message::Response, maybe. Don't think String is enough.
            let msg = Message::Response(op.op_id, "Hello there".to_string(), res);

            let buf = to_vec(&msg).unwrap();
            // self.sock.send_to(&buf, addr).unwrap();
            self.handler.network().send(*addr, &buf);
        }
    }
}

/// This is the main loop for the replica. It listens for messages from the leaders and clients.
pub fn listen(id: NodeId, listener: NodeListener<()>, handler: NodeHandler<()>) {
    let mut rep = Replica::new(id, handler.clone());
    let leaders = get_all_leaders(handler, &rep.db);
    let params = Params::new();
    // println!("Inited replica {id}.");
    let mut nt = listener.for_each_async(move |event| match event.network() {
        NetEvent::Message(endpoint, buf) => {
            let msg = from_slice::<Message>(&buf).unwrap();
            // dbg!(&msg);
            match msg {
                Message::Request(c) => {
                    let c = c.clone();
                    let _ = rep.clients.try_insert(c.client_id, endpoint);
                    rep.requests.push(c);
                    // dbg!(&rep.requests);
                }
                Message::Decision(slot, command) => {
                    // Accept the consensus.
                    rep.decisions.insert(slot, command);
                    while let Some(c1) = rep.decisions.get(&rep.slot_out) {
                        if let Some(c2) = rep.proposals.remove(&rep.slot_out) {
                            if c2 != *c1 {
                                rep.requests.push(c2);
                            }
                        }

                        // Actually do the thing.
                        rep.perform(c1.clone()); // GAH, CLONES!
                    }
                    // dbg!(&rep.decisions);
                    if rep.decisions.len() >= params.k {
                        // println!("{id} returning");
                        rep.handler.stop();
                        return; // Timing.
                    }
                }
                Message::Identify(kind, ip, other_id, reply) => {
                    let Ok(_) = remember_node(&rep.db, other_id, &ip.to_string(), kind) else {
                        panic!("WTF.");
                    };

                    if reply {
                        let msg = Message::Identify(Identity::Replica, local_ip().unwrap(), id, false);
                        let buf = to_vec(&msg).unwrap();
                        rep.handler.network().send(endpoint, &buf);
                    }
                }
                _ => unreachable!(), // It had better be, damn it.
            }
            rep.propose(&leaders);
        }
        NetEvent::Connected(_ep, _) => {
            // println!("Replica {id} Connected to {ep}.");
        }
        NetEvent::Accepted(_ep, _) => {
            // println!("Replica {id} Accepted {ep}.");
        }
        NetEvent::Disconnected(_ep) => {
            // println!("Replica {id} Disconnected from {ep}.");
        }
    });

    nt.wait();
}
