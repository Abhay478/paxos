use std::net::SocketAddr;

use local_ip_address::local_ip;
use message_io::{
    adapters::udp::UdpConnectConfig, network::{Endpoint, Transport}, node::{self, NodeHandler, NodeListener}
};
use serde_json::to_vec;
use sqlite::{Connection, Row, State};

use crate::{Entry, Identity, NodeId};

use super::{leader::Agent, Ballot, Message};

pub const LEADER_PORT: u16 = 4000;
pub const SCOUT_PORT: u16 = 4500;
pub const COMMANDER_PORT: u16 = 5000;
pub const REPLICA_PORT: u16 = 6000;
pub const ACCEPTOR_PORT: u16 = 8000;
pub const CLIENT_PORT: u16 = 9000;
pub const IDENTIFY_PORT: u16 = 3000;

// Right now this is only used for the count. If I don't find any other uses for it, I'll change to count(*)
pub const LOCAL_NODES_QUERY: &'static str = "SELECT * FROM nodes WHERE ip = :ip AND kind = :kind;";
pub const GLOBAL_NODES_QUERY: &'static str = "SELECT * FROM nodes WHERE kind = :kind;";

pub const REMEMBER: &'static str = "INSERT INTO nodes VALUES (:id, :ip, :kind, :port);";
pub const ALL_NODES: &'static str = "SELECT (ip, port) FROM nodes;";

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
    q.bind(&[(":ip", &*lip), (":kind", &*kind.to_string())][..])
        .unwrap();
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

fn declare_self(
    db: &Connection,
    id: NodeId,
    kind: Identity,
    port: u16,
) -> Result<SocketAddr, sqlite::Error> {
    let lip = local_ip().unwrap();
    let mut q = db.prepare(REMEMBER).unwrap();
    q.bind((":id", &id.id[..])).unwrap();
    q.bind((":ip", &*lip.to_string())).unwrap();
    q.bind((":kind", &*kind.to_string())).unwrap();
    q.bind((":port", port as i64)).unwrap();

    q.next()?;
    Ok(SocketAddr::from((lip, port)))
}

pub(crate) fn remember_node(
    db: &Connection,
    &Entry { id, kind, addr }: &Entry,
) -> Result<State, sqlite::Error> {
    let mut q = db.prepare(REMEMBER).unwrap();
    q.bind((":id", &id.id[..])).unwrap();
    q.bind((":ip", &*addr.ip().to_string())).unwrap();
    q.bind((":kind", &*kind.to_string())).unwrap();
    q.bind((":port", addr.port() as i64)).unwrap();

    q.next()
}

fn identify(entry: Entry) {
    let msg = Message::Identify(entry, false);
    let buf = to_vec(&msg).unwrap();

    // This is a one-time node pair that is used only for the Identify operation.
    // Dropped at the end of the function.
    // Contains the socket address for the persistent listener.
    let (handler, _) = node::split::<()>();

    let addr = "255.255.255.255:3000".parse::<SocketAddr>().unwrap();
    // All the Identify's, broadcasted to port 3000. At the receiver, it writes to db and sends it's details to `addr`.
    let config = UdpConnectConfig::default().with_broadcast();
    let ep = handler.network().connect_with(message_io::network::TransportConnect::Udp(config), addr).unwrap();
    handler.network().send(ep.0, &buf);

}

pub(crate) fn teach(entry: Entry) {
    let addr = entry.addr;
    let msg = Message::Identify(entry, false);
    let buf = to_vec(&msg).unwrap();
    // Might remove extra node.
    let (handler, _) = node::split::<()>();
    let ep = handler.network().connect(Transport::Udp, addr).unwrap();
    handler.network().send(ep.0, &buf);
}

pub fn replica_init(
    id: NodeId,
    db: &Connection,
) -> Result<(NodeHandler<()>, NodeListener<()>, SocketAddr), sqlite::Error> {
    let out = node::split::<()>();
    let local_things = get_all_local_nodes(db, Identity::Replica);
    let port = REPLICA_PORT + local_things.len() as u16;
    let addr = declare_self(db, id, Identity::Replica, port)?;
    out.0
        .network()
        .listen(
            Transport::Udp,
            addr,
        )
        .unwrap();

        out.0
        .network()
        .listen(
            Transport::Udp,
            SocketAddr::from((local_ip().unwrap(), IDENTIFY_PORT)),
        )
        .unwrap();

    identify(Entry::new(id, Identity::Replica, addr));
    Ok((out.0, out.1, addr))
}

pub fn leader_init(
    id: NodeId,
    db: &Connection,
) -> Result<(NodeHandler<Agent>, NodeListener<Agent>, SocketAddr), sqlite::Error> {
    let out = node::split();
    let local_things = get_all_local_nodes(db, Identity::Leader);
    let port = LEADER_PORT + local_things.len() as u16;

    let addr = declare_self(db, id, Identity::Leader, port)?;
    out.0
        .network()
        .listen(
            Transport::Udp,
            addr,
        )
        .unwrap();

        out.0
        .network()
        .listen(
            Transport::Udp,
            SocketAddr::from((local_ip().unwrap(), IDENTIFY_PORT)),
        )
        .unwrap();

        identify(Entry::new(id, Identity::Leader, addr));
    Ok((out.0, out.1, addr))
}

pub fn acceptor_init(
    id: NodeId,
    db: &Connection,
) -> Result<(NodeHandler<()>, NodeListener<()>, SocketAddr), sqlite::Error> {
    let out = node::split::<()>();
    let local_things = get_all_local_nodes(db, Identity::Acceptor);
    let port = ACCEPTOR_PORT + local_things.len() as u16;
    let addr = declare_self(db, id, Identity::Acceptor, port)?;
    out.0
        .network()
        .listen(
            Transport::Udp,
            addr,
        )
        .unwrap();

    out.0
        .network()
        .listen(
            Transport::Udp,
            SocketAddr::from((local_ip().unwrap(), IDENTIFY_PORT)),
        )
        .unwrap();

    identify(Entry::new(id, Identity::Acceptor, addr));
    Ok((out.0, out.1, addr))
}

pub fn get_all_leaders<Y>(handler: NodeHandler<Y>, db: &Connection) -> Vec<Endpoint> {
    let all_nodes = get_all_global_nodes(db, Identity::Leader);
    all_nodes
        .into_iter()
        .map(|i| {
            // let temp = node::split::<()>();
            let ip = i.read::<&str, _>("ip").as_bytes();
            let ip_only = [ip[0], ip[1], ip[2], ip[3]];

            let port = i.read::<i64, _>("port");
            let out = handler
                .network()
                .connect(Transport::Udp, SocketAddr::from((ip_only, port as u16)))
                .unwrap();
            // dbg!(&out);
            out.0
        })
        .collect()

    // todo!()
}

pub fn get_all_replicas<Y>(handler: NodeHandler<Y>, db: &Connection) -> Vec<Endpoint> {
    let all_nodes = get_all_global_nodes(db, Identity::Replica);
    all_nodes
        .into_iter()
        .map(|i| {
            let ip = i.read::<&str, _>("ip").as_bytes();
            let ip_only = [ip[0], ip[1], ip[2], ip[3]];

            let port = i.read::<i64, _>("port");
            let out = handler
                .network()
                .connect(Transport::Udp, SocketAddr::from((ip_only, port as u16)))
                .unwrap();
            // dbg!(&out);
            out.0
        })
        .collect()
}

pub fn get_all_acceptors<Y>(handler: NodeHandler<Y>, db: &Connection) -> Vec<Endpoint> {
    let all_nodes = get_all_global_nodes(db, Identity::Acceptor);
    all_nodes
        .into_iter()
        .map(|i| {
            let ip = i.read::<&str, _>("ip").as_bytes();
            let ip_only = [ip[0], ip[1], ip[2], ip[3]];

            let port = i.read::<i64, _>("port");
            let out = handler
                .network()
                .connect(Transport::Udp, SocketAddr::from((ip_only, port as u16)))
                .unwrap();
            // dbg!(&out);
            out.0
        })
        .collect()
}
