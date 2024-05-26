use std::net::SocketAddr;

use local_ip_address::local_ip;
use message_io::{
    network::{Endpoint, Transport},
    node::{self, NodeHandler, NodeListener},
};
use sqlite::{Connection, Row, State};

use crate::{Identity, NodeId};

use super::{leader::Agent, Ballot};

pub const LEADER_PORT: u16 = 4000;
pub const SCOUT_PORT: u16 = 4500;
pub const COMMANDER_PORT: u16 = 5000;
pub const REPLICA_PORT: u16 = 6000;
pub const ACCEPTOR_PORT: u16 = 8000;
pub const CLIENT_PORT: u16 = 9000;

pub const LOCAL_NODES_QUERY: &'static str = "SELECT * FROM nodes WHERE ip = :ip AND kind = :kind;";
pub const GLOBAL_NODES_QUERY: &'static str = "SELECT * FROM nodes WHERE kind = :kind;";
pub const DECLARE_SELF: &'static str = "INSERT INTO nodes (id, ip, kind) VALUES (:id, :ip, :kind);";

/* pub const LEADER_COUNT: u8 = 3;
pub const REPLICA_COUNT: u8 = 3;
pub const ACCEPTOR_COUNT: u8 = 3; */

pub fn scout_init(acceptors: &Vec<Endpoint>) -> (NodeHandler<Ballot>, NodeListener<Ballot>) {
    let (scout_h, scout_l) = node::split();
    for a in acceptors.iter() {
        scout_h.network().connect(Transport::Udp, a.addr()).unwrap();
    }
    (scout_h, scout_l)
}

pub fn commander_init(acceptors: &Vec<Endpoint>) -> (NodeHandler<()>, NodeListener<()>) {
    let (commander_h, commander_l) = node::split();
    for a in acceptors.iter() {
        commander_h
            .network()
            .connect(Transport::Udp, a.addr())
            .unwrap();
    }
    (commander_h, commander_l)
}

pub fn client_init() -> (NodeHandler<()>, NodeListener<()>) {
    let out = node::split::<()>();
    out
}


fn get_all_local_nodes(db: &Connection, kind: Identity) -> Vec<Row> {
    let lip = local_ip().unwrap().to_string();
    let mut q = db.prepare(LOCAL_NODES_QUERY).unwrap();
    q.bind(&[(":ip", &*lip), (":kind", &*kind.to_string())][..]).unwrap();
    let out = q.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>();
    // Do we need to do anything more?
    out
}

fn get_all_global_nodes(db: &Connection, kind: Identity) -> Vec<Row> {
    let mut q = db.prepare(GLOBAL_NODES_QUERY).unwrap();
    q.bind((":kind", &*kind.to_string())).unwrap();
    let out = q.into_iter().map(|x| x.unwrap()).collect::<Vec<_>>();
    // Do we need to do anything more?
    out
}

fn declare_self(db: &Connection, id: NodeId, kind: Identity) -> Result<State, sqlite::Error> {
    let lip = local_ip().unwrap().to_string();
    let mut q = db.prepare(DECLARE_SELF).unwrap();
    q.bind((":id", &id.id[..]))
        .unwrap();
    q.bind((":ip", &*lip))
        .unwrap();
    q.bind((":kind", &*kind.to_string())).unwrap();

    q.next()
}

pub(crate) fn remember_node(db: &Connection, id: NodeId, ip: &str, kind: Identity) -> Result<State, sqlite::Error> {
    let mut q = db.prepare(DECLARE_SELF).unwrap();
    q.bind((":id", &id.id[..]))
        .unwrap();
    q.bind((":ip", &*ip))
        .unwrap();
    q.bind((":kind", &*kind.to_string())).unwrap();

    q.next()
}

pub fn replica_init(id: NodeId, db: &Connection) -> Result<(NodeHandler<()>, NodeListener<()>), sqlite::Error> {
    let out = node::split::<()>();
    let local_things = get_all_local_nodes(db, Identity::Replica);

    out.0
        .network()
        .listen(
            Transport::Udp,
            SocketAddr::from((local_ip().unwrap(), REPLICA_PORT + local_things.len() as u16)),
        )
        .unwrap();

    declare_self(db, id, Identity::Replica)?;
    Ok(out)
}

pub fn leader_init(id: NodeId, db: &Connection) -> Result<(NodeHandler<Agent>, NodeListener<Agent>), sqlite::Error> {
    let out = node::split();
    let local_things = get_all_local_nodes(db, Identity::Leader);

    out.0
        .network()
        .listen(
            Transport::Udp,
            SocketAddr::from((local_ip().unwrap(), LEADER_PORT + local_things.len() as u16)),
        )
        .unwrap();

    declare_self(db, id, Identity::Leader)?;
    Ok(out)
}

pub fn acceptor_init(id: NodeId, db: &Connection) -> Result<(NodeHandler<()>, NodeListener<()>), sqlite::Error> {
    let out = node::split::<()>();
    let local_things = get_all_local_nodes(db, Identity::Acceptor);
    out.0
        .network()
        .listen(
            Transport::Udp,
            SocketAddr::from((local_ip().unwrap(), ACCEPTOR_PORT + local_things.len() as u16)),
        )
        .unwrap();

    declare_self(db, id, Identity::Acceptor)?;
    Ok(out)
}

pub fn get_all_leaders<Y>(handler: NodeHandler<Y>, db: &Connection) -> Vec<Endpoint> {
    let all_nodes = get_all_global_nodes(db, Identity::Leader);
    all_nodes.into_iter()
        .map(|i| {
            // let temp = node::split::<()>();
            let ip = i.read::<&str, _>("ip").as_bytes();
            let ip_only = [ip[0], ip[1], ip[2], ip[3]];

            let port = i.read::<i64, _>("port");
            let out = handler
                .network()
                .connect(
                    Transport::Udp,
                    SocketAddr::from((ip_only, port as u16)),
                )
                .unwrap();
            // dbg!(&out);
            out.0
        })
        .collect()

    // todo!()
}

pub fn get_all_replicas<Y>(handler: NodeHandler<Y>, db: &Connection) -> Vec<Endpoint> {
    let all_nodes = get_all_global_nodes(db, Identity::Replica);
    all_nodes.into_iter()
        .map(|i| {
            let ip = i.read::<&str, _>("ip").as_bytes();
            let ip_only = [ip[0], ip[1], ip[2], ip[3]];

            let port = i.read::<i64, _>("port");
            let out = handler
                .network()
                .connect(
                    Transport::Udp,
                    SocketAddr::from((ip_only, port as u16)),
                )
                .unwrap();
            // dbg!(&out);
            out.0
        })
        .collect()
}

pub fn get_all_acceptors<Y>(handler: NodeHandler<Y>, db: &Connection) -> Vec<Endpoint> {
    let all_nodes = get_all_global_nodes(db, Identity::Acceptor);
    all_nodes.into_iter()
        .map(|i| {
            let ip = i.read::<&str, _>("ip").as_bytes();
            let ip_only = [ip[0], ip[1], ip[2], ip[3]];

            let port = i.read::<i64, _>("port");
            let out = handler
                .network()
                .connect(
                    Transport::Udp,
                    SocketAddr::from((ip_only, port as u16)),
                )
                .unwrap();
            // dbg!(&out);
            out.0
        })
        .collect()
}
