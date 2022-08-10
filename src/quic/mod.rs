//! Core routing table implementation fine-tuned for QUIC.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use time::OffsetDateTime;

use crate::core::{
    id::Id,
    message::{Message, Response},
    routing_table::RoutingTable,
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
    conn_id: Option<ConnId>,
    conn_state: ConnState,
    stream_state: StreamState,
    last_seen: Option<OffsetDateTime>,
}

impl QuicMeta {
    fn new(
        addr: SocketAddr,
        conn_id: Option<ConnId>,
        conn_state: ConnState,
        stream_state: StreamState,
        last_seen: Option<OffsetDateTime>,
    ) -> Self {
        Self {
            addr,
            conn_id,
            conn_state,
            stream_state,
            last_seen,
        }
    }
}

/// The core routing table implementation.
pub struct QuicRouter {
    rt: RoutingTable<ConnId, QuicMeta>,
}

impl QuicRouter {
    fn insert(&mut self, id: Id, addr: SocketAddr) -> bool {
        if id == self.rt.local_id {
            return false;
        }

        self.rt
            .peer_list
            .entry(id)
            .and_modify(|meta| {
                // Only modify the address if we are not currently connected to this peer,
                // hopefully this is sufficient to guard against malicious peer lists.
                if matches!(meta.conn_state, ConnState::Connected) {
                    meta.addr = addr;
                }
            })
            .or_insert_with(|| {
                QuicMeta::new(
                    addr,
                    None,
                    ConnState::Disconnected,
                    StreamState::Closed,
                    None,
                )
            });

        true
    }

    // TODO: consider CanConnect enum as return type here.
    fn can_connect(&mut self, id: Id) -> (bool, Option<u32>) {
        let i = match self.rt.local_id.log2_distance(&id) {
            Some(i) => i,
            None => return (false, None),
        };

        // TODO: check if identifier is already in use?
        //
        // TODO: eviction policy.
        //
        // 1. Check `last_seen` for each peer in the bucket if is full and select a candidate for
        //    eviction.
        // 2. Send PING expecting PONG to the candidate, if no response comes back or the stream
        //    has closed, disconnect and replace with the new identifier.
        let bucket = self.rt.buckets.entry(i).or_insert_with(HashSet::new);
        match bucket.len().cmp(&self.rt.max_bucket_size.into()) {
            Ordering::Less => {
                // Bucket still has space. Signal the value could be inserted into the bucket (once
                // the connection is succesful).
                (true, Some(i))
            }
            Ordering::Equal => {
                // Bucket is full. Signal the value can't currently be inserted into the bucket.
                (false, None)
            }
            Ordering::Greater => {
                // Bucket is over capacity, this should never happen.
                unreachable!()
            }
        }
    }

    fn set_connected(&mut self, id: Id, conn_id: ConnId, stream_state: StreamState) -> bool {
        match self.can_connect(id) {
            (true, Some(i)) => {
                if let (Some(quic_meta), Some(bucket)) =
                    (self.rt.peer_list.get_mut(&id), self.rt.buckets.get_mut(&i))
                {
                    // If the bucket insert returns `false`, it means the id is already in the bucket and the
                    // peer is connected.
                    let _res = bucket.insert(id);
                    debug_assert!(_res);

                    self.rt.id_list.insert(conn_id, id);
                    quic_meta.conn_id = Some(conn_id);
                    quic_meta.conn_state = ConnState::Connected;
                    quic_meta.last_seen = Some(OffsetDateTime::now_utc());
                }

                true
            }
            _ => false,
        }
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
