//! Core routing table implementation.

use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use rand::{seq::IteratorRandom, thread_rng, Fill};
use time::OffsetDateTime;

use crate::{
    id::Id,
    message::{Chunk, FindKNodes, KNodes, Message, Ping, Pong, Response},
    traits::ProcessData,
};

#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "sync")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "sync")))]
pub use sync::AsyncRoutingTable;

const K: u8 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnState {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy)]
struct PeerMeta {
    listening_addr: SocketAddr,
    conn_addr: Option<SocketAddr>,
    conn_state: ConnState,
    last_seen: Option<OffsetDateTime>,
}

impl PeerMeta {
    fn new(
        listening_addr: SocketAddr,
        conn_addr: Option<SocketAddr>,
        conn_state: ConnState,
        last_seen: Option<OffsetDateTime>,
    ) -> Self {
        Self {
            listening_addr,
            conn_addr,
            conn_state,
            last_seen,
        }
    }
}

/// The core routing table implementation.
#[derive(Debug, Clone)]
pub struct RoutingTable {
    // The node's local identifier.
    local_id: Id,
    // The maximum number of identifiers that can be contained in a bucket.
    max_bucket_size: u8,
    // The buckets constructed for broadcast purposes (only contains connected identifiers).
    buckets: HashMap<u32, HashSet<Id>>,
    // Maps identifiers to peer meta data (connected and disconnected).
    peer_list: HashMap<Id, PeerMeta>,
    // Maps peer addresses to peer identifiers (connected only).
    id_list: HashMap<SocketAddr, Id>,
}

impl Default for RoutingTable {
    fn default() -> Self {
        let mut rng = thread_rng();
        let mut bytes = [0u8; Id::BYTES];
        debug_assert!(bytes.try_fill(&mut rng).is_ok());

        Self {
            local_id: Id::new(bytes),
            max_bucket_size: K,
            buckets: HashMap::new(),
            peer_list: HashMap::new(),
            id_list: HashMap::new(),
        }
    }
}

impl RoutingTable {
    /// Creates a new routing table.
    pub fn new(local_id: Id, max_bucket_size: u8) -> Self {
        Self {
            local_id,
            max_bucket_size,
            ..Default::default()
        }
    }

    /// Returns this router's local identifier.
    pub fn local_id(&self) -> Id {
        self.local_id
    }

    /// Returns the identifier corresponding to the address, if it exists.
    pub fn peer_id(&self, addr: SocketAddr) -> Option<Id> {
        self.id_list.get(&addr).copied()
    }

    /// Returns `true` if the record exists already or was inserted, `false` if an attempt was made to
    /// insert our local identifier.
    pub fn insert(
        &mut self,
        id: Id,
        listening_addr: SocketAddr,
        conn_addr: Option<SocketAddr>,
    ) -> bool {
        // Buckets should only contain connected peers. The other structures should track
        // connection state.

        // An insert can happen in two instances:
        //
        // 1. the peer initiated the connection (should only be inserted into the bucket if there
        //    is space, this requires a later call to set_connected).
        // 2. the peer was included in a list from another peer (should be inserted as
        //    disconnected unless it is already in the list and is connected).
        //
        // Insert all peers as disconnected initially. The caller can then check if the
        // bucket has space and if so initiates a connection in case 1, or accepts the connection
        // in case 2.
        //
        // If a peer exists already, we update the address information without reseting the
        // connecton state. The node wrapping this implementation should make sure to call
        // `set_disconnected` if a connection is closed or dropped.

        if id == self.local_id {
            return false;
        }

        self.peer_list
            .entry(id)
            .and_modify(|meta| {
                meta.listening_addr = listening_addr;
                meta.conn_addr = conn_addr;
            })
            .or_insert_with(|| {
                PeerMeta::new(listening_addr, conn_addr, ConnState::Disconnected, None)
            });

        self.id_list.insert(listening_addr, id);
        if let Some(addr) = conn_addr {
            self.id_list.insert(addr, id);
        }

        true
    }

    /// Returns whether or not there is space in the bucket corresponding to the identifier and the
    /// appropriate bucket index if there is.
    pub fn can_connect(&mut self, conn_addr: SocketAddr) -> (bool, Option<u32>) {
        let id = match self.peer_id(conn_addr) {
            Some(id) => id,
            None => return (false, None),
        };

        let i = match self.local_id().log2_distance(&id) {
            Some(i) => i,
            None => return (false, None),
        };

        let bucket = self.buckets.entry(i).or_insert_with(HashSet::new);
        match bucket.len().cmp(&self.max_bucket_size.into()) {
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

    /// Sets the peer as connected on the routing table, returning `false` if there is no room to
    /// connect the peer.
    pub fn set_connected(&mut self, conn_addr: SocketAddr) -> bool {
        match (self.can_connect(conn_addr), self.peer_id(conn_addr)) {
            ((true, Some(i)), Some(id)) => {
                if let (Some(peer_meta), Some(bucket)) =
                    (self.peer_list.get_mut(&id), self.buckets.get_mut(&i))
                {
                    // If the bucket insert returns `false`, it means the id is already in the bucket and the
                    // peer is connected.
                    debug_assert!(bucket.insert(id));
                    peer_meta.conn_state = ConnState::Connected;
                    peer_meta.last_seen = Some(OffsetDateTime::now_utc());
                }

                true
            }

            _ => false,
        }
    }

    /// Removes an identifier from the buckets, sets the peer to disconnected.
    pub fn set_disconnected(&mut self, conn_addr: SocketAddr) {
        let id = match self.peer_id(conn_addr) {
            Some(id) => id,
            None => return,
        };

        let i = match self.local_id().log2_distance(&id) {
            Some(i) => i,
            None => return,
        };

        if let Some(bucket) = self.buckets.get_mut(&i) {
            bucket.remove(&id);
        }

        if let Some(peer_meta) = self.peer_list.get_mut(&id) {
            peer_meta.conn_state = ConnState::Disconnected;
            // Remove the entry from the identifier list as the addr is likely to change when a
            // peer reconnects later (also this means we only have one collection tracking
            // disconnected peers for simplicity).
            self.id_list.remove(&peer_meta.listening_addr);
        }
    }

    /// Selects the broadcast peers for a particular height, returns `None` if the broadcast
    /// shouldn't continue any further.
    pub fn select_broadcast_peers(&self, height: u32) -> Option<Vec<(u32, SocketAddr)>> {
        let mut rng = thread_rng();

        // Don't broadcast any further.
        if height == 0 {
            return None;
        }

        let mut selected_peers = vec![];
        for h in 0..height {
            if let Some(bucket) = self.buckets.get(&h) {
                // Choose one peer at random per bucket.
                if let Some(id) = bucket.iter().choose(&mut rng) {
                    // The value should exist as the peer is in the bucket.
                    let peer_meta = self.peer_list.get(id);
                    debug_assert!(peer_meta.is_some());
                    debug_assert_eq!(peer_meta.unwrap().conn_state, ConnState::Connected);
                    // Return the connection address, not the listening address as we need to
                    // broadcast through the connection, not to the listener.
                    debug_assert!(peer_meta.unwrap().conn_addr.is_some());
                    let addr = peer_meta.unwrap().conn_addr.unwrap();

                    selected_peers.push((h, addr))
                }
            }
        }

        Some(selected_peers)
    }

    /// Returns the K closest nodes to the identifier.
    fn find_k_closest(&self, id: &Id, k: usize) -> Vec<(Id, SocketAddr)> {
        // There is a total order over the id-space, though we take the log2 of the XOR distance,
        // and so peers within a bucket are considered at the same distance. We use an unstable
        // sort as we don't care if items at the same distance are reordered (and it is usually
        // faster).
        let mut ids: Vec<_> = self
            .peer_list
            .iter()
            .map(|(&candidate_id, &candidate_meta)| (candidate_id, candidate_meta.listening_addr))
            .collect();
        // TODO: bench and consider sort_by_cached_key.
        ids.sort_unstable_by_key(|(candidate_id, _)| candidate_id.log2_distance(id));
        ids.truncate(k);

        ids
    }

    // MESSAGE PROCESSING

    /// Processes a peer's message. If it is a query, an appropriate response is returned to
    /// be sent.
    pub fn process_message<S: Clone, T: ProcessData<S>>(
        &mut self,
        state: S,
        message: Message,
        source: SocketAddr,
    ) -> Option<Response> {
        let id = match self.peer_id(source) {
            Some(id) => id,
            None => return None,
        };

        // Update the peer's last seen timestamp.
        if let Some(peer_meta) = self.peer_list.get_mut(&id) {
            peer_meta.last_seen = Some(OffsetDateTime::now_utc())
        }

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
                if let Some(broadcast) = self.process_chunk::<S, T>(state, chunk) {
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
        // Prepare a response, send back the same nonce so the original sender can identify the
        // request the response corresponds to.
        Pong {
            nonce: ping.nonce,
            id: self.local_id(),
        }
    }

    fn process_pong(&mut self, _pong: Pong) {
        // TODO: how should latency factor into the broadcast logic? Perhaps keep a table with the
        // message nonces for latency calculation?
    }

    fn process_find_k_nodes(&self, find_k_nodes: FindKNodes) -> KNodes {
        let k_closest_nodes = self.find_k_closest(&find_k_nodes.id, K as usize);

        KNodes {
            nonce: find_k_nodes.nonce,
            nodes: k_closest_nodes,
        }
    }

    fn process_k_nodes(&mut self, k_nodes: KNodes) {
        // Save the new peer information.
        for (id, addr) in k_nodes.nodes {
            self.insert(id, addr, None);
        }

        // TODO: work out who to connect with to continue the recursive search. Should this be
        // continual or only when bootstrapping the network?
    }

    fn process_chunk<S: Clone, T: ProcessData<S>>(
        &self,
        state: S,
        chunk: Chunk,
    ) -> Option<Vec<(SocketAddr, Chunk)>> {
        // Cheap as the backing storage is shared amongst instances.
        let data = chunk.data.clone();
        let data_as_t: T = chunk.data.into();
        let is_kosher = data_as_t.verify_data(state.clone());

        // This is where the buckets come in handy. When a node processes a chunk message, it
        // selects peers in buckets ]h, 0] and propagates the CHUNK message. If h = 0, no
        // propagation occurs.
        if !is_kosher {
            return None;
        }

        data_as_t.process_data(state);

        // TODO: return the wrapped data as well as the peers to propagate to.
        self.select_broadcast_peers(chunk.height).map(|v| {
            v.iter()
                .map(|(height, addr)| {
                    (
                        *addr,
                        Chunk {
                            // TODO: work out if this is a bad idea.
                            nonce: chunk.nonce,
                            height: *height,
                            data: data.clone(),
                        },
                    )
                })
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert() {
        let mut rt = RoutingTable::new(Id::from_u16(0), 1);

        // Attempt to insert our local id.
        assert!(!rt.insert(rt.local_id, "127.0.0.1:0".parse().unwrap(), None));

        // ... 0001 -> bucket i = 0
        assert!(rt.insert(Id::from_u16(1), "127.0.0.1:1".parse().unwrap(), None));

        // ... 0010 -> bucket i = 1
        assert!(rt.insert(Id::from_u16(2), "127.0.0.1:2".parse().unwrap(), None));

        // ... 0011 -> bucket i = 1
        // This should still return true, since no peers have been inserted into the buckets yet
        // and there is still space.
        assert!(rt.insert(Id::from_u16(3), "127.0.0.1:3".parse().unwrap(), None));
    }

    #[test]
    fn set_connected() {
        // Set the max bucket size to a low value so we can easily test when it's full.
        let mut rt = RoutingTable::new(Id::from_u16(0), 1);

        // ... 0001 -> bucket i = 0
        let addr = "127.0.0.1:1".parse().unwrap();
        rt.insert(Id::from_u16(1), addr, Some(addr));
        assert!(rt.set_connected(addr));

        // ... 0010 -> bucket i = 1
        let addr = "127.0.0.1:2".parse().unwrap();
        rt.insert(Id::from_u16(2), addr, Some(addr));
        assert!(rt.set_connected(addr));

        // ... 0011 -> bucket i = 1
        let addr = "127.0.0.1:3".parse().unwrap();
        rt.insert(Id::from_u16(3), addr, Some(addr));
        assert!(!rt.set_connected(addr));
    }

    #[test]
    fn find_k_closest() {
        let mut rt = RoutingTable::new(Id::from_u16(0), 5);

        // Generate 5 identifiers and addressses.
        let peers: Vec<(Id, SocketAddr)> = (1..=5)
            .into_iter()
            .map(|i| {
                (
                    Id::from_u16(i as u16),
                    format!("127.0.0.1:{}", i).parse().unwrap(),
                )
            })
            .collect();

        for peer in &peers {
            assert!(rt.insert(peer.0, peer.1, Some(peer.1)));
            assert!(rt.set_connected(peer.1));
        }

        let k = 3;
        let k_closest = rt.find_k_closest(&rt.local_id, k);

        assert_eq!(k_closest.len(), 3);
        assert!(k_closest.contains(&peers[0]));
        assert!(k_closest.contains(&peers[1]));
        assert!(k_closest.contains(&peers[2]));
    }

    #[test]
    fn select_broadcast_peers() {
        let mut rt = RoutingTable::new(Id::from_u16(0), 5);

        // Generate 5 identifiers and addressses.
        let peers: Vec<(Id, SocketAddr)> = (1..=5)
            .into_iter()
            .map(|i| {
                (
                    Id::from_u16(i as u16),
                    format!("127.0.0.1:{}", i).parse().unwrap(),
                )
            })
            .collect();

        for peer in peers {
            // Conn address is listening address (all peers received the connections).
            assert!(rt.insert(peer.0, peer.1, Some(peer.1)));
            assert!(rt.set_connected(peer.1));
        }

        // Find the random addresses in each bucket.

        // If the height is 0, we are the last node in the recursion, don't broadcast.
        let h = 0;
        assert!(rt.select_broadcast_peers(h).is_none());

        let h = 1;
        // Should be present.
        let selected_peers = rt.select_broadcast_peers(h).unwrap();
        assert_eq!(selected_peers.len(), 1);
        // Height for selected peer should be 0.
        assert_eq!(selected_peers[0].0, 0);

        // TODO: Bucket at index 0 should contain the id corresponding to the address.
    }
}
