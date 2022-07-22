use std::net::SocketAddr;

use bincode::{Decode, Encode};
use bytes::Bytes;
use time::OffsetDateTime;

use crate::router::RoutingTable;

const K: u8 = 20;

pub type Nonce = u128;
type Id = u128;
type Height = u32;

pub enum Response {
    Unicast(Message),
    Broadcast(Vec<(SocketAddr, Message)>),
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub enum Message {
    Ping(Ping),
    Pong(Pong),

    FindKNodes(FindKNodes),
    KNodes(KNodes),

    Chunk(Chunk),
}

impl Message {
    pub fn nonce(&self) -> Option<Nonce> {
        match self {
            Message::Ping(ping) => Some(ping.nonce),
            Message::Pong(pong) => Some(pong.nonce),
            Message::FindKNodes(find_k_nodes) => Some(find_k_nodes.nonce),
            Message::KNodes(k_nodes) => Some(k_nodes.nonce),
            Message::Chunk(_chunk) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Ping {
    pub nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Pong {
    pub nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct FindKNodes {
    pub nonce: Nonce,
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct KNodes {
    nonce: Nonce,
    nodes: Vec<(Id, SocketAddr)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct Chunk {
    height: Height,
    #[bincode(with_serde)]
    data: Bytes,
}

impl RoutingTable {
    pub fn process_message(&mut self, message: Message) -> Option<Response> {
        match message {
            Message::Ping(ping) => {
                let pong = self.process_ping(ping);
                Some(Response::Unicast(Message::Pong(pong)))
            }
            Message::Pong(pong) => {
                self.process_pong(pong);
                None
            }
            Message::FindKNodes(find_k_nodes) => {
                let k_nodes = self.process_find_k_nodes(find_k_nodes);
                Some(Response::Unicast(Message::KNodes(k_nodes)))
            }
            Message::KNodes(k_nodes) => {
                self.process_k_nodes(k_nodes);
                None
            }
            Message::Chunk(chunk) => {
                if let Some(broadcast) = self.process_chunk(chunk) {
                    let broadcast = broadcast
                        .into_iter()
                        .map(|(addr, message)| (addr, Message::Chunk(message)))
                        .collect();

                    Some(Response::Broadcast(broadcast))
                } else {
                    None
                }
            }
        }
    }

    fn process_ping(&mut self, ping: Ping) -> Pong {
        // The peer should already exist in the peer list and be present in the bucket, update
        // the last_seen timestamp.
        self.set_last_seen(ping.id, OffsetDateTime::now_utc());

        // Prepare a response, send back the same nonce so the original sender can identify the
        // request the response corresponds to.
        Pong {
            nonce: ping.nonce,
            id: self.local_id(),
        }
    }

    fn process_pong(&mut self, pong: Pong) {
        // Update the last seen timestamp.
        self.set_last_seen(pong.id, OffsetDateTime::now_utc());

        // TODO: how should latency factor into the broadcast logic? Perhaps keep a table with the
        // message nonces for latency calculation?
    }

    fn process_find_k_nodes(&self, find_k_nodes: FindKNodes) -> KNodes {
        // TODO: update last seen?
        let k_closest_nodes = self.find_k_closest(find_k_nodes.id, K as usize);

        KNodes {
            nonce: find_k_nodes.nonce,
            nodes: k_closest_nodes,
        }
    }

    fn process_k_nodes(&mut self, k_nodes: KNodes) {
        // Save the new peer information.
        for (id, addr) in k_nodes.nodes {
            self.insert(id, addr);
        }

        // TODO: work out who to connect with to continue the recursive search. Should this be
        // continual or only when bootstrapping the network?
    }

    fn process_chunk(&self, chunk: Chunk) -> Option<Vec<(SocketAddr, Chunk)>> {
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
        self.select_broadcast_peers(chunk.height).map(|v| {
            v.iter()
                .map(|(height, addr)| {
                    (
                        *addr,
                        Chunk {
                            height: *height,
                            // Cheap as the backing storage is shared amongst instances.
                            data: chunk.data.clone(),
                        },
                    )
                })
                .collect()
        })
    }
}
