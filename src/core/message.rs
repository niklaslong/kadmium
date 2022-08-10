//! Protocol message types.

use std::net::SocketAddr;

#[cfg(feature = "codec")]
use bincode::{Decode, Encode};
use bytes::Bytes;

use crate::core::id::Id;

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
    pub fn variant_as_str(&self) -> &str {
        match self {
            Message::Ping(_) => "ping",
            Message::Pong(_) => "pong",
            Message::FindKNodes(_) => "find_k_nodes",
            Message::KNodes(_) => "k_nodes",
            Message::Chunk(_) => "chunk",
        }
    }

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
        matches!(self, Message::Pong(_) | Message::KNodes(_))
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

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};

    use super::*;

    #[test]
    fn variant_as_str() {
        assert_eq!(
            Message::Ping(Ping {
                nonce: 0,
                id: Id::from_u16(0)
            })
            .variant_as_str(),
            "ping"
        );
        assert_eq!(
            Message::Pong(Pong {
                nonce: 0,
                id: Id::from_u16(0)
            })
            .variant_as_str(),
            "pong"
        );
        assert_eq!(
            Message::FindKNodes(FindKNodes {
                nonce: 0,
                id: Id::from_u16(0)
            })
            .variant_as_str(),
            "find_k_nodes"
        );
        assert_eq!(
            Message::KNodes(KNodes {
                nonce: 0,
                nodes: vec![]
            })
            .variant_as_str(),
            "k_nodes"
        );
        assert_eq!(
            Message::Chunk(Chunk {
                nonce: 0,
                height: 0,
                data: Bytes::new()
            })
            .variant_as_str(),
            "chunk"
        );
    }

    #[test]
    fn nonce() {
        let mut rng = thread_rng();
        let nonce = rng.gen();

        assert_eq!(
            Message::Ping(Ping {
                nonce,
                id: Id::from_u16(0)
            })
            .nonce(),
            nonce
        );
        assert_eq!(
            Message::Pong(Pong {
                nonce,
                id: Id::from_u16(0)
            })
            .nonce(),
            nonce
        );
        assert_eq!(
            Message::FindKNodes(FindKNodes {
                nonce,
                id: Id::from_u16(0)
            })
            .nonce(),
            nonce
        );
        assert_eq!(
            Message::KNodes(KNodes {
                nonce,
                nodes: vec![]
            })
            .nonce(),
            nonce
        );
        assert_eq!(
            Message::Chunk(Chunk {
                nonce,
                height: 0,
                data: Bytes::new()
            })
            .nonce(),
            nonce
        );
    }

    #[test]
    fn is_response() {
        // RESPONSES
        assert!(Message::Pong(Pong {
            nonce: 0,
            id: Id::from_u16(0)
        })
        .is_response());
        assert!(Message::KNodes(KNodes {
            nonce: 0,
            nodes: vec![]
        })
        .is_response());
        // NOT RESPONSES
        assert!(!Message::Ping(Ping {
            nonce: 0,
            id: Id::from_u16(0)
        })
        .is_response());
        assert!(!Message::FindKNodes(FindKNodes {
            nonce: 0,
            id: Id::from_u16(0)
        })
        .is_response());
        assert!(!Message::Chunk(Chunk {
            nonce: 0,
            height: 0,
            data: Bytes::new()
        })
        .is_response());
    }
}
