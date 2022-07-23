//! Protocol message types.

use std::net::SocketAddr;

#[cfg(feature = "codec")]
use bincode::{Decode, Encode};
use bytes::Bytes;

use crate::router::Id;

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
    pub height: Height,

    #[cfg_attr(feature = "codec", bincode(with_serde))]
    pub data: Bytes,
}
