//! Core routing table implementation fine-tuned for QUIC.

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use time::OffsetDateTime;

use crate::{
    id::Id,
    message::{Message, Response},
    traits::ProcessData,
};

type ConnId = usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnState {
    Connected,
    Disconnected,
}

enum StreamState {
    Closed,
    Uni,
    Bi,
}

struct QuicMeta {
    addr: SocketAddr,
    conn_id: ConnId,
    conn_state: ConnState,
    stream_state: StreamState,
    last_seen: Option<OffsetDateTime>,
}

/// The core routing table implementation.
pub struct RoutingTable {
    // The node's local identifier.
    local_id: Id,
    // The maximum number of identifiers that can be contained in a bucket.
    max_bucket_size: u8,
    // The number of addresses to share when responding to a FIND_K_NODES query.
    k: u8,
    // The buckets constructed for broadcast purposes (only contains connected identifiers).
    buckets: HashMap<u32, HashSet<Id>>,
    // Maps identifiers to peer meta data (connected and disconnected).
    pub(crate) peer_list: HashMap<Id, QuicMeta>,
    // Maps peer connection identifiers (QUIC) to peer identifiers.
    id_list: HashMap<ConnId, Id>,
}

impl RoutingTable {
    fn insert(&mut self, id: Id, addr: SocketAddr) -> bool {
        todo!()
    }

    fn set_connected(&mut self, id: Id, conn_id: ConnId, stream_state: StreamState) -> bool {
        todo!()
    }

    fn set_disconnected(&mut self, conn_id: ConnId) -> bool {
        todo!()
    }

    fn select_broadcast_peers(&self, height: u32) -> Option<Vec<(u32, SocketAddr)>> {
        todo!()
    }

    fn process_message<S: Clone, T: ProcessData<S>>(
        &mut self,
        state: S,
        message: Message,
        conn_id: ConnId,
    ) -> Option<Response> {
        todo!()
    }
}
