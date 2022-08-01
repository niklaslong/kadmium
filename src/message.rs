//! Protocol message types.

use std::net::SocketAddr;

#[cfg(feature = "codec")]
use bincode::{Decode, Encode};
use bytes::Bytes;

use crate::id::Id;

pub type Nonce = u128;
type Height = u32;

pub enum Response {
    Unicast(Message),
    Broadcast(Vec<(SocketAddr, Message)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub enum Message {
    Ping(Ping),
    Pong(Pong),

    FindKNodes(FindKNodes),
    KNodes(KNodes),

    Chunk(Chunk),
}

impl Message {
    pub fn nonce(&self) -> Nonce {
        match self {
            Message::Ping(ping) => ping.nonce,
            Message::Pong(pong) => pong.nonce,
            Message::FindKNodes(find_k_nodes) => find_k_nodes.nonce,
            Message::KNodes(k_nodes) => k_nodes.nonce,
            Message::Chunk(chunk) => chunk.nonce,
        }
    }

    pub fn is_response(&self) -> bool {
        match self {
            Message::Pong(_) | Message::KNodes(_) => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub struct Ping {
    pub nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub struct Pong {
    pub nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub struct FindKNodes {
    pub nonce: Nonce,
    pub id: Id,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub struct KNodes {
    pub nonce: Nonce,
    pub nodes: Vec<(Id, SocketAddr)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub struct Chunk {
    // TODO: work out if this is a bad idea.
    pub nonce: Nonce,
    pub height: Height,

    #[cfg_attr(feature = "codec", bincode(with_serde))]
    pub data: Bytes,
}
