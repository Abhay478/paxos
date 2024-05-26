#![allow(dead_code)]
use std::{net::SocketAddr, time::Duration};

use hashbrown::HashMap;
use message_io::{
    events::TimerId,
    network::{Endpoint, NetEvent, Transport},
    node::{self, NodeEvent, NodeHandler},
};
use rand::distributions::{Distribution, Uniform};
use serde_json::{from_slice, to_vec};

use crate::{Params, ReplicaState};

use super::{
    dir::get_peers, Campaign, Heartbeat, Log, Message, Replicate, Reply, ServerState, Timer,
};

pub struct Server {
    id: usize,
    state: ServerState, // Look at enum variants
    rst: ReplicaState,  // State of the replica
    current_term: usize,
    voted_for: Option<usize>,           // Leader election.
    log: Vec<Log>,                      // Replica<index, Log<term, ACTUAL SHIT>>
    commit_index: usize,                // index of highest committed entry
    last_applied: usize,                // index of highest applied entry
    next_index: HashMap<usize, usize>,  // index of next log entry to send to each server
    match_index: HashMap<usize, usize>, // index of highest log entry known to be replicated on server

    u: rand::distributions::Uniform<f64>,
    handler: NodeHandler<Timer>,
    peers: HashMap<usize, Endpoint>,
    clients: HashMap<SocketAddr, Endpoint>,
    current_timer: Option<TimerId>,
    pending: Vec<Message>,
}

impl Server {
    fn new(id: usize, peers: HashMap<usize, Endpoint>, handler: NodeHandler<Timer>) -> Self {
        let mut out = Self {
            id,
            state: ServerState::Follower,
            rst: ReplicaState::default(),
            current_term: 0,
            voted_for: None,
            log: vec![Log {
                term: 0,
                command: None,
            }],
            commit_index: 0,
            last_applied: 0,
            next_index: peers
                .iter()
                .map(|(a, _b)| (*a, 1))
                .collect::<HashMap<_, _>>(),
            match_index: peers
                .iter()
                .map(|(a, _b)| (*a, 0))
                .collect::<HashMap<_, _>>(),
            u: Uniform::new(150.0, 300.0),
            handler,
            peers,
            clients: HashMap::new(),
            current_timer: None,
            pending: vec![],
        };

        // Start the timeouts.
        out.reset_timeout();
        out.reset_heartbeat();
        out
    }

    fn empty_decree(&self) {
        let hb = Heartbeat {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index: self.log.len() - 1,
            prev_log_term: self.log.last().unwrap().term,
            leader_commit: self.commit_index,
        };

        for p in self.peers.iter() {
            self.handler.network().send(
                *p.1,
                &to_vec(&Message::Heartbeat(Replicate {
                    hb,
                    entries: vec![],
                }))
                .unwrap(),
            );
        }
        self.reset_heartbeat();
    }

    fn decree(&self) {
        let mut hb = Heartbeat {
            term: self.current_term,
            leader_id: self.id,
            prev_log_index: 0,
            prev_log_term: 0,
            leader_commit: self.commit_index,
        };

        for p in self.peers.iter() {
            let entries = self
                .log
                .iter()
                .enumerate()
                .skip(self.next_index[p.0] - 1)
                .map(|x| (x.0, x.1.clone()))
                .collect::<Vec<_>>();
            hb.prev_log_index = self.next_index[p.0] - 1;
            hb.prev_log_term = self.log[hb.prev_log_index].term;
            self.handler.network().send(
                *p.1,
                &to_vec(&Message::Heartbeat(Replicate { hb, entries })).unwrap(),
            );
        }
        self.reset_heartbeat();
    }

    fn campaign(&mut self) {
        self.current_term += 1;
        self.voted_for = Some(self.id);
        self.state = ServerState::Candidate(1);

        let cp = Campaign {
            term: self.current_term,
            candidate_id: self.id,
            last_log_index: self.log.len() - 1,
            last_log_term: self.log.last().unwrap().term,
        };

        for p in self.peers.iter() {
            self.handler
                .network()
                .send(*p.1, &to_vec(&Message::Campaign(cp.clone())).unwrap());
        }
        self.reset_timeout();
    }

    fn crown(&mut self) {
        self.state = ServerState::Leader;
        self.voted_for = None;
        // println!("Crowned {}", self.id);
        self.empty_decree();
        for (_a, b) in self.next_index.iter_mut() {
            *b = self.log.len();
        }
    }

    fn reset_timeout(&mut self) {
        let rng = &mut rand::thread_rng();
        self.current_timer = Some(self.handler.signals().send_with_timer(
            Timer::Election,
            Duration::from_millis(self.u.sample(rng) as u64),
        ));
    }

    fn reset_heartbeat(&self) {
        self.handler
            .signals()
            .send_with_timer(Timer::Heartbeat, Duration::from_millis(50));
    }

    fn reject(&mut self, ep: Endpoint) {
        let rep = &Message::ServerReply(Reply {
            from: self.id,
            success: false,
            term: self.current_term,
        });
        self.handler.network().send(ep, &to_vec(rep).unwrap());
        self.voted_for = None;
    }

    fn vote(&mut self, ep: Endpoint, c: Campaign) {
        if let Some(t) = self.current_timer {
            self.handler.signals().cancel_timer(t);
        }
        self.current_term = c.term;
        self.voted_for = Some(c.candidate_id);
        self.state = ServerState::Follower;
        let rep = &Message::ServerReply(Reply {
            from: self.id,
            success: true,
            term: self.current_term,
        });
        self.handler.network().send(ep, &to_vec(rep).unwrap());
        self.reset_timeout();
    }

    fn accept(&self, ep: Endpoint) {
        let rep = &Message::ServerReply(Reply {
            from: self.id,
            success: true,
            term: self.current_term,
        });
        self.handler.network().send(ep, &to_vec(rep).unwrap());
    }

    fn perform(&mut self) {
        for q in self.last_applied + 1..=self.commit_index {
            // perform
            let cmd = self.log[q].command.clone();
            if cmd.is_none() {
                continue;
            }
            let cmd = cmd.unwrap();
            let (state, _res) = ReplicaState::triv(cmd.op.clone())(&self.rst);
            self.rst = state;
            if self.state == ServerState::Leader {
                let sock = cmd.client;
                if let Some(ep) = self.clients.get(&sock) {
                    self.handler
                        .network()
                        .send(*ep, &to_vec(&Message::Response(cmd.clone())).unwrap());
                } else {
                    let ep = self
                        .handler
                        .network()
                        .connect(Transport::Udp, sock)
                        .unwrap()
                        .0;
                    self.clients.insert(sock, ep);
                    self.handler
                        .network()
                        .send(ep, &to_vec(&Message::Response(cmd.clone())).unwrap());
                };
            }
        }
        self.last_applied = self.commit_index;
    }
}

pub fn run(id: usize, addr: SocketAddr) {
    let params = Params::new();
    let (handler, listener) = node::split::<Timer>();
    handler.network().listen(Transport::Udp, addr).unwrap();
    let peers = get_peers(id, handler.clone());

    let mut server = Server::new(id, peers, handler);
    // println!("Server {id} up.");
    let mut nt = listener.for_each_async(move |event| {
        match event {
            NodeEvent::Network(e) => {
                match e {
                    NetEvent::Connected(_ep, _) => {
                        // println!("Raft server {id} Connected to {ep}.");
                    }
                    NetEvent::Accepted(_ep, _) => {
                        // println!("Raft server {id} Accepted {ep}.");
                    }
                    NetEvent::Disconnected(_ep) => {
                        // println!("Raft server {id} Disconnected from {ep}.");
                    }
                    NetEvent::Message(ep, buf) => {
                        let msg = from_slice::<Message>(&buf);
                        if let Ok(msg) = msg {
                            match msg {
                                // If leader, decree. Else, redirect to leader.
                                Message::Request(ref cmd) => {
                                    // dbg!(&msg);
                                    match server.state {
                                        ServerState::Follower => {
                                            if let Some(l) = server.voted_for {
                                                // dbg!(id, l);
                                                let leader = server.peers[&l];
                                                server.handler.network().send(leader, &buf);
                                            } else {
                                                server.pending.push(msg);
                                            }
                                        },
                                        ServerState::Candidate(_) => {
                                            server.pending.push(msg);
                                        },
                                        ServerState::Leader => {
                                            server.log.push(Log {
                                                term: server.current_term,
                                                command: Some(cmd.clone()),
                                            });
                                            server.decree();
                                        },
                                    }
                                }

                                // A server can never receive a response.
                                // The leader responds to the client directly.
                                // The client socket address is contained in the command.
                                Message::Response(_) => unreachable!(),
                                // Add to log
                                Message::Heartbeat(rep) => {
                                    // println!("HB, {}", rep.entries.len());
                                    // Old leader
                                    if rep.hb.term < server.current_term {
                                        // println!("{}@{} Rejected {}@{}", id, server.current_term, rep.hb.leader_id, rep.hb.term);
                                        server.reject(ep);
                                    } else if server.log.len() - 1 < rep.hb.prev_log_index // Old log, send previous stuff also
                                    || server.log[rep.hb.prev_log_index].term != rep.hb.prev_log_term // Log conflict, send previous stuff also
                                    {
                                        // So that pending messages are not lost.
                                        server.current_term = rep.hb.term;
                                        server.voted_for = Some(rep.hb.leader_id);
                                        // println!("{} unmerge {}", id, rep.hb.leader_id);
                                        server.reject(ep);
                                    } else {
                                        // println!("{} Accepted {}", id, rep.hb.leader_id);
                                        server.current_term = rep.hb.term;
                                        if let Some(t) = server.current_timer {
                                            server.handler.signals().cancel_timer(t);
                                        }
                                        server.state = ServerState::Follower;
                                        server.voted_for = Some(rep.hb.leader_id);
                                        for (i, l) in rep.entries.iter() {
                                            if *i < server.log.len() {
                                                // Override
                                                server.log[*i] = l.clone();
                                            } else {
                                                // New
                                                server.log.push(l.clone());
                                            }
                                        }
                                        
                                        while !server.pending.is_empty() {
                                            let msg = server.pending.pop().unwrap();
                                            let leader = server.peers[&server.voted_for.unwrap()];
                                            server
                                                .handler
                                                .network()
                                                .send(leader, &to_vec(&msg).unwrap());
                                        }
                                    }

                                    if rep.entries.len() > 0 {
                                        // dbg!(&server.log);
                                    }
    
                                    if rep.hb.leader_commit > server.commit_index {
                                        server.commit_index =
                                            rep.hb.leader_commit.min(server.log.len() - 1);
                                        if server.commit_index > server.last_applied {
                                            // perform
                                            server.perform();
                                        }
                                    }
    
                                    server.accept(ep);
                                    server.reset_timeout();
                                }
                                // Candidacy
                                Message::Campaign(c) => {
                                    // Somehow, we don't need to check ServerState?
    
                                    // If we at newer term, reply false.
                                    if c.term < server.current_term {
                                        server.reject(ep);
                                    }
                                    // Ambiguity here. Should we abstain?
                                    else if c.term > server.current_term
                                        || c.last_log_index >= server.log.len() - 1
                                    {
                                        // Can be optimised, not doing it.
                                        if let Some(s) = server.voted_for {
                                            if s == c.candidate_id {
                                                server.vote(ep, c);
                                            }
                                        } else if server.voted_for.is_none() {
                                            server.vote(ep, c);
                                        }
                                    }
                                }
    
                                Message::ServerReply(res) => {
                                    match server.state {
                                        ServerState::Follower => {
                                            // println!("BAD.");
                                        }
                                        // Votes
                                        ServerState::Candidate(v) => {
                                            if res.success {
                                                // Majority
                                                if v > (server.peers.len() + 1) / 2 {
                                                    server.crown();
                                                } else {
                                                    server.state = ServerState::Candidate(v + 1);
                                                }
                                            } else {
                                                server.current_term = res.term;
                                                server.state = ServerState::Follower;
                                                server.voted_for = None
                                            }
                                        }
                                        // Rejects
                                        ServerState::Leader => {
                                            if res.term > server.current_term {
                                                server.current_term = res.term;
                                                server.state = ServerState::Follower;
                                                server.voted_for = None;
                                                // todo!()
                                            } else {
                                                if res.success {
                                                    // TODO: Verify.
                                                    server
                                                        .match_index
                                                        .insert(res.from, server.log.len() - 1);
                                                    server
                                                        .next_index
                                                        .insert(res.from, server.log.len());
    
                                                    for i in (server.commit_index + 1..server.log.len())
                                                        .rev()
                                                    {
                                                        let mut count = 1;
                                                        for (_a, b) in server.match_index.iter() {
                                                            if *b >= i {
                                                                count += 1;
                                                            }
                                                        }
                                                        if count > (server.peers.len() + 1) / 2 {
                                                            server.commit_index = i;
                                                            break;
                                                        }
                                                    }
    
                                                    if server.commit_index > server.last_applied {
                                                        server.perform();
                                                    }
                                                } else {
                                                    // Term matches, log does not.
                                                    let u = server.next_index.get_mut(&res.from).unwrap();
                                                    if *u > 0 {
                                                        *u -= 1;
                                                    }
                                                    server.decree();
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            NodeEvent::Signal(t) => {
                if server.log.len() > params.k {
                    // dbg!(server.log.len(), params.k);
                    server.handler.stop();
                    return;
                }
                match t {
                Timer::Heartbeat => {
                    if server.state == ServerState::Leader {
                        server.empty_decree();
                    }
                }
                Timer::Election => {
                    // println!("Campaign {id}");
                    server.campaign();
                }
            }},
        }
    });

    nt.wait();
}
