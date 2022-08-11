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
pub enum ConnState {
    Connected,
    Disconnected,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum StreamState {
    Closed,
    Uni,
    Bi,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct QuicMeta {
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

/// The core router implementation.
pub struct QuicRouter {
    /// The routing table backing this router.
    rt: RoutingTable<ConnId, QuicMeta>,
}

impl QuicRouter {
    pub fn new(local_id: Id, max_bucket_size: u8, k: u8) -> Self {
        Self {
            rt: RoutingTable {
                local_id,
                max_bucket_size,
                k,
                buckets: HashMap::new(),
                peer_list: HashMap::new(),
                id_list: HashMap::new(),
            },
        }
    }

    pub fn local_id(&self) -> Id {
        self.rt.local_id
    }

    pub fn peer_meta(&self, id: &Id) -> Option<QuicMeta> {
        self.rt.peer_list.get(id).copied()
    }

    pub fn insert(&mut self, id: Id, addr: SocketAddr) -> bool {
        if id == self.rt.local_id {
            return false;
        }

        self.rt
            .peer_list
            .entry(id)
            .and_modify(|meta| {
                // Only modify the address if we are not currently connected to this peer,
                // hopefully this is sufficient to guard against malicious peer lists.
                if matches!(meta.conn_state, ConnState::Disconnected) {
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
    pub fn can_connect(&mut self, id: Id) -> (bool, Option<u32>) {
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

    pub fn set_connected(&mut self, id: Id, conn_id: ConnId, stream_state: StreamState) -> bool {
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

    pub fn set_disconnected(&mut self, conn_id: ConnId) -> bool {
        todo!()
    }

    pub fn select_broadcast_peers(&self, height: u32) -> Option<Vec<(u32, SocketAddr)>> {
        todo!()
    }

    pub fn process_message<S: Clone, T: ProcessData<S>>(
        &mut self,
        state: S,
        message: Message,
        conn_id: ConnId,
    ) -> Option<Response> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Produces a local address from the supplied port.
    // TODO: consider testing with random ports and IDs.
    fn localhost_with_port(port: u16) -> SocketAddr {
        format!("127.0.0.1:{}", port).parse().unwrap()
    }

    #[test]
    fn insert() {
        let mut router = QuicRouter::new(Id::from_u16(0), 1, 20);
        // ... 0001 -> bucket i = 0
        assert!(router.insert(Id::from_u16(1), localhost_with_port(1)));
        // ... 0010 -> bucket i = 1
        assert!(router.insert(Id::from_u16(2), localhost_with_port(2)));
        // ... 0011 -> bucket i = 1
        assert!(router.insert(Id::from_u16(3), localhost_with_port(3)));
    }

    #[test]
    fn insert_defaults() {
        let mut router = QuicRouter::new(Id::from_u16(0), 1, 20);

        let id = Id::from_u16(1);
        let addr = localhost_with_port(1);

        assert!(router.insert(id, addr));

        let meta = router.peer_meta(&id).unwrap();
        assert_eq!(meta.addr, addr);
        assert_eq!(meta.conn_id, None);
        assert_eq!(meta.conn_state, ConnState::Disconnected);
        assert_eq!(meta.stream_state, StreamState::Closed);
        assert_eq!(meta.last_seen, None);
    }

    #[test]
    fn insert_self() {
        let mut router = QuicRouter::new(Id::from_u16(0), 1, 20);
        // Attempt to insert our local id.
        assert!(!router.insert(router.local_id(), localhost_with_port(0)));
    }

    #[test]
    fn insert_duplicate() {
        let mut router = QuicRouter::new(Id::from_u16(0), 1, 20);
        // The double insert will still return true.
        assert!(router.insert(Id::from_u16(1), localhost_with_port(1)));
        assert!(router.insert(Id::from_u16(1), localhost_with_port(1)));
    }

    #[test]
    fn insert_duplicate_updates_addr() {
        let mut router = QuicRouter::new(Id::from_u16(0), 1, 20);

        let id = Id::from_u16(1);
        let addr = localhost_with_port(1);
        assert!(router.insert(id, addr));
        assert_eq!(router.peer_meta(&id).unwrap().addr, addr);

        // Inserting the same identifier with a different address should result in the new address
        // getting stored (so long as the peer is disconnected).
        let id = Id::from_u16(1);
        let addr = localhost_with_port(2);
        assert!(router.insert(id, addr));
        assert_eq!(router.peer_meta(&id).unwrap().addr, addr);
    }
}
