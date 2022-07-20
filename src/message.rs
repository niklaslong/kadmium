use std::net::SocketAddr;

use bincode::{Decode, Encode};
use time::OffsetDateTime;

use crate::tree::RoutingTable;

const K: u8 = 20;

type Nonce = u128;
type Id = u128;
type Height = u32;

pub struct RawData;

#[derive(Encode, Decode)]
pub enum Message<T> {
    Ping(Ping),
    Pong(Pong),

    FindKNodes(FindKNodes),
    KNodes(KNodes),

    Chunk(Chunk<T>),
}

#[derive(Encode, Decode)]
pub struct Ping {
    pub nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    pub id: Id,
}

#[derive(Encode, Decode)]
pub struct Pong {
    _nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    id: Id,
}

#[derive(Encode, Decode)]
pub struct FindKNodes {
    nonce: Nonce,
    id: Id,
}

#[derive(Encode, Decode)]
pub struct KNodes {
    _nonce: Nonce,
    nodes: Vec<(Id, SocketAddr)>,
}

#[derive(Encode, Decode)]
pub struct Chunk<T> {
    height: Height,
    data: T,
}

impl RoutingTable {
    //     pub fn process_message(&mut self, message: Message) -> Option<Response> {
    //         match message {
    //             Message::Ping(ping) => Unicast(self.process_ping(ping)),
    //             Message::Pong(pong) => {
    //                 self.process_pong(pong);
    //                 None
    //             }
    //             Message::FindKNodes(find_k_nodes) => Unicast(self.process_find_k_nodes(find_k_nodes)),
    //             Message::KNodes(k_nodes) => {
    //                 self.process_k_nodes(k_nodes);
    //                 None
    //             }
    //             _ => todo!(),
    //         }
    //     }

    pub fn process_ping(&mut self, ping: Ping) -> Pong {
        // The peer should already exist in the peer list and be present in the bucket, update
        // the last_seen timestamp.
        self.set_last_seen(ping.id, OffsetDateTime::now_utc());

        // Prepare a response, send back the same nonce so the original sender can identify the
        // request the response corresponds to.
        Pong {
            _nonce: ping.nonce,
            id: self.local_id,
        }
    }

    pub fn process_pong(&mut self, pong: Pong) {
        // Update the last seen timestamp.
        self.set_last_seen(pong.id, OffsetDateTime::now_utc());

        // TODO: how should latency factor into the broadcast logic? Perhaps keep a table with the
        // message nonces for latency calculation?
    }

    pub fn process_find_k_nodes(&self, find_k_nodes: FindKNodes) -> KNodes {
        // TODO: update last seen?
        let k_closest_nodes = self.find_k_closest(find_k_nodes.id, K as usize);

        KNodes {
            _nonce: find_k_nodes.nonce,
            nodes: k_closest_nodes,
        }
    }

    pub fn process_k_nodes(&mut self, k_nodes: KNodes) {
        // Save the new peer information.
        for (id, addr) in k_nodes.nodes {
            self.insert(id, addr);
        }

        // TODO: work out who to connect with to continue the recursive search. Should this be
        // continual or only when bootstrapping the network?
    }

    pub fn process_chunk<T>(&self, chunk: Chunk<T>) -> Option<Vec<(Height, SocketAddr)>> {
        // TODO: verify data, perhaps accept a function as a parameter, should return true or false
        // for data verification.
        let is_kosher = true;

        // This is where the buckets come in handy. When a node processes a chunk message, it
        // selects peers in buckets ]h, 0] and propagates the CHUNK message. If h = 0, no
        // propagation occurs.
        if !is_kosher {
            return None;
        }

        // TODO: return the wrapped data as well as the peers to propagate to.
        self.select_broadcast_peers(chunk.height)
    }
}
