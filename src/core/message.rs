//! Protocol message types.

use std::net::SocketAddr;

#[cfg(feature = "codec")]
use bincode::{Decode, Encode};
use bytes::Bytes;

use crate::core::id::Id;

/// A NONCE identifies a query/response pair or a specific broadcast.
pub type Nonce = u128;
type Height = u32;

/// Specifies what should be done in response to a message.
pub enum Response {
    /// The caller should respond to the sender with the wrapped message.
    Unicast(Message),
    /// The caller should respond by broadcasting the wrapped message to the specified peer
    /// addresses.
    Broadcast(Vec<(SocketAddr, Message)>),
}

/// Kadcast message variants.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
pub enum Message {
    Init(Init),
    /// PING messages requires a PONG response, useful to measure connection latency and peer
    /// liveness.
    Ping(Ping),
    /// PONG is the correct response to PING, it must contain the same NONCE.
    Pong(Pong),

    /// FIND_NODES messages query alpha peers for their K closest nodes to an identifier.
    FindKNodes(FindKNodes),
    /// NODES is the correct response to FIND_NODES, it must contain the same NONCE.
    KNodes(KNodes),

    /// CHUNK is used to broadcast data to the network. It is also the correct response to a
    /// REQUEST message (TODO).
    Chunk(Chunk),
}

impl Message {
    pub fn variant_as_str(&self) -> &str {
        match self {
            Message::Init(_) => "init",
            Message::Ping(_) => "ping",
            Message::Pong(_) => "pong",
            Message::FindKNodes(_) => "find_k_nodes",
            Message::KNodes(_) => "k_nodes",
            Message::Chunk(_) => "chunk",
        }
    }

    pub fn nonce(&self) -> Nonce {
        match self {
            Message::Init(init) => init.nonce,
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
#[cfg_attr(test, derive(Default))]
pub struct Init {
    pub nonce: Nonce,
    pub id: Id,
    pub port: u16,
}

/// The data making up a PING message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
#[cfg_attr(test, derive(Default))]
pub struct Ping {
    pub nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    pub id: Id,
}

/// The data making up a PONG message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
#[cfg_attr(test, derive(Default))]
pub struct Pong {
    pub nonce: Nonce,
    // TODO: sending the ID here may not be necessary.
    pub id: Id,
}

/// The data making up a FIND_NODES message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
#[cfg_attr(test, derive(Default))]
pub struct FindKNodes {
    pub nonce: Nonce,
    pub id: Id,
}

/// The data making up a NODES message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
#[cfg_attr(test, derive(Default))]
pub struct KNodes {
    pub nonce: Nonce,
    pub nodes: Vec<(Id, SocketAddr)>,
}

/// The data making up a CHUNK message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "codec", derive(Encode, Decode))]
#[cfg_attr(test, derive(Default))]
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

    macro_rules! assert_as_str {
        ($variant:ident, $variant_as_str:expr) => {
            let message = Message::$variant($variant::default());
            assert_eq!(message.variant_as_str(), $variant_as_str);
        };
    }

    #[test]
    fn variant_as_str() {
        assert_as_str!(Init, "init");
        assert_as_str!(Ping, "ping");
        assert_as_str!(Pong, "pong");
        assert_as_str!(FindKNodes, "find_k_nodes");
        assert_as_str!(KNodes, "k_nodes");
        assert_as_str!(Chunk, "chunk");
    }

    macro_rules! assert_nonce {
        ($($variant:ident),*) => {
            // Reuse the same RNG for all nonce tests.
            let mut rng = thread_rng();

            $(
                let nonce = rng.gen();
                // Set the nonce on the message payload and wrap in the enum.
                let mut variant = $variant::default();
                variant.nonce = nonce;
                let message = Message::$variant(variant);

                // Check `Message::nonce` returns the same nonce.
                assert_eq!(message.nonce(), nonce);
            )*
        };
    }

    #[test]
    fn nonce() {
        assert_nonce!(Init, Ping, Pong, FindKNodes, KNodes, Chunk);
    }

    macro_rules! assert_is_response {
        ($($variant:ident),*) => {
            $(
                assert!(Message::$variant($variant::default()).is_response());
            )*
        };
    }

    macro_rules! assert_is_not_response {
        ($($variant:ident),*) => {
            $(
                assert!(!Message::$variant($variant::default()).is_response());
            )*
        };
    }

    #[test]
    fn is_response() {
        assert_is_response!(Pong, KNodes);
        assert_is_not_response!(Init, Ping, FindKNodes, Chunk);
    }
}
