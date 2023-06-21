use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use rand::{seq::IteratorRandom, thread_rng, Fill};
use time::OffsetDateTime;

use crate::core::{
    id::Id,
    message::{Chunk, FindKNodes, KNodes, Message, Ping, Pong, Response},
    routing_table::RoutingTable,
    traits::ProcessData,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnState {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Copy)]
pub struct TcpMeta {
    pub(crate) listening_addr: SocketAddr,
    pub(crate) conn_addr: Option<SocketAddr>,
    pub(crate) conn_state: ConnState,
    pub(crate) last_seen: Option<OffsetDateTime>,
}

impl TcpMeta {
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

type ConnAddr = SocketAddr;

/// The core router implementation.
#[derive(Debug, Clone)]
pub struct TcpRouter {
    /// The routing table backing this router.
    rt: RoutingTable<ConnAddr, TcpMeta>,
}

impl Default for TcpRouter {
    fn default() -> Self {
        let mut rng = thread_rng();
        let mut bytes = [0u8; Id::BYTES];
        let _res = bytes.try_fill(&mut rng);
        // Sanity check this doesn't fail in debug mode.
        debug_assert!(_res.is_ok());

        Self {
            rt: RoutingTable {
                local_id: Id::new(bytes),
                max_bucket_size: 20,
                k: 20,
                buckets: HashMap::new(),
                peer_list: HashMap::new(),
                id_list: HashMap::new(),
            },
        }
    }
}

impl TcpRouter {
    /// Creates a new router.
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

    #[cfg(feature = "sync")]
    pub(crate) fn routing_table(&self) -> &RoutingTable<ConnAddr, TcpMeta> {
        &self.rt
    }

    /// Returns this router's local identifier.
    pub fn local_id(&self) -> Id {
        self.rt.local_id
    }

    /// Returns the identifier corresponding to the address, if it exists.
    pub fn peer_id(&self, addr: SocketAddr) -> Option<Id> {
        self.rt.id_list.get(&addr).copied()
    }

    /// Returns the peer's metadata if it exists.
    pub fn peer_meta(&self, id: &Id) -> Option<&TcpMeta> {
        self.rt.peer_list.get(id)
    }

    /// Returns `true` if the record exists already or was inserted, `false` if an attempt was made to
    /// insert our local identifier.
    pub fn insert(&mut self, id: Id, listening_addr: SocketAddr) -> bool {
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

        if id == self.rt.local_id {
            return false;
        }

        self.rt
            .peer_list
            .entry(id)
            .and_modify(|meta| {
                meta.listening_addr = listening_addr;
            })
            .or_insert_with(|| TcpMeta::new(listening_addr, None, ConnState::Disconnected, None));

        self.rt.id_list.insert(listening_addr, id);

        true
    }

    /// Returns whether or not there is space in the bucket corresponding to the identifier and the
    /// appropriate bucket index if there is.
    pub fn can_connect(&mut self, id: Id) -> (bool, Option<u32>) {
        let i = match self.local_id().log2_distance(&id) {
            Some(i) => i,
            None => return (false, None),
        };

        // TODO: check if identifier is already in use?

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

    /// Sets the peer as connected on the router, returning `false` if there is no room to
    /// connect the peer.
    pub fn set_connected(&mut self, id: Id, conn_addr: SocketAddr) -> bool {
        match self.can_connect(id) {
            (true, Some(i)) => {
                if let (Some(peer_meta), Some(bucket)) =
                    (self.rt.peer_list.get_mut(&id), self.rt.buckets.get_mut(&i))
                {
                    // If the bucket insert returns `false`, it means the id is already in the bucket and the
                    // peer is connected.
                    let _res = bucket.insert(id);
                    debug_assert!(_res);

                    self.rt.id_list.insert(conn_addr, id);
                    peer_meta.conn_addr = Some(conn_addr);
                    peer_meta.conn_state = ConnState::Connected;
                    peer_meta.last_seen = Some(OffsetDateTime::now_utc());
                }

                true
            }

            _ => false,
        }
    }

    /// Removes an identifier from the buckets, sets the peer to disconnected and returns `true` on
    /// success, `false` otherwise.
    pub fn set_disconnected(&mut self, conn_addr: SocketAddr) -> bool {
        let id = match self.peer_id(conn_addr) {
            Some(id) => id,
            None => return false,
        };

        let i = self
            .local_id()
            .log2_distance(&id)
            .expect("self can't have an identifier in the peer list");

        if let (Some(peer_meta), Some(bucket)) =
            (self.rt.peer_list.get_mut(&id), self.rt.buckets.get_mut(&i))
        {
            let bucket_res = bucket.remove(&id);
            debug_assert!(bucket_res);

            // Remove the entry from the identifier list as the addr is likely to change when a
            // peer reconnects later (also this means we only have one collection tracking
            // disconnected peers for simplicity).
            let id_list_res = self
                .rt
                .id_list
                .remove(&peer_meta.conn_addr.expect("conn_addr must be present"));
            debug_assert!(id_list_res.is_some());

            peer_meta.conn_addr = None;
            peer_meta.conn_state = ConnState::Disconnected;

            return bucket_res && id_list_res.is_some();
        }

        false
    }

    /// Returns `true` if the address is connected, `false` if it isn't.
    pub fn is_connected(&self, addr: SocketAddr) -> bool {
        if let Some(id) = self.peer_id(addr) {
            if let Some(peer_meta) = self.peer_meta(&id) {
                return matches!(peer_meta.conn_state, ConnState::Connected);
            }
        }

        false
    }

    /// Returns the list disconnected addresses.
    pub fn disconnected_addrs(&self) -> Vec<SocketAddr> {
        self.rt
            .peer_list
            .iter()
            .filter(|(_, &peer_meta)| matches!(peer_meta.conn_state, ConnState::Disconnected))
            .map(|(_, &peer_meta)| peer_meta.listening_addr)
            .collect()
    }

    /// Returns the list connected addresses.
    pub fn connected_addrs(&self) -> Vec<SocketAddr> {
        self.rt
            .peer_list
            .iter()
            .filter(|(_, &peer_meta)| matches!(peer_meta.conn_state, ConnState::Connected))
            .map(|(_, &peer_meta)| peer_meta.conn_addr.unwrap())
            .collect()
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
            if let Some(bucket) = self.rt.buckets.get(&h) {
                // Choose one peer at random per bucket.
                if let Some(id) = bucket.iter().choose(&mut rng) {
                    // The value should exist as the peer is in the bucket.
                    let peer_meta = self.rt.peer_list.get(id);
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
            .rt
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
        conn_addr: SocketAddr,
    ) -> Option<Response> {
        let id = match self.peer_id(conn_addr) {
            Some(id) => id,
            None => return None,
        };

        // Update the peer's last seen timestamp.
        if let Some(peer_meta) = self.rt.peer_list.get_mut(&id) {
            peer_meta.last_seen = Some(OffsetDateTime::now_utc())
        }

        match message {
            Message::Init(_init) => None,
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
        let k_closest_nodes = self.find_k_closest(&find_k_nodes.id, self.rt.k as usize);

        KNodes {
            nonce: find_k_nodes.nonce,
            nodes: k_closest_nodes,
        }
    }

    fn process_k_nodes(&mut self, k_nodes: KNodes) {
        // Save the new peer information.
        for (id, listening_addr) in k_nodes.nodes {
            self.insert(id, listening_addr);
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

        let data_as_t: T = match chunk.data.try_into() {
            Ok(data) => data,
            Err(_) => return None,
        };

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

    // Produces a local address from the supplied port.
    // TODO: consider testing with random ports and IDs.
    fn localhost_with_port(port: u16) -> SocketAddr {
        format!("127.0.0.1:{port}").parse().unwrap()
    }

    #[test]
    fn default() {
        let router = TcpRouter::default();
        // We can't assert on randomness, but we can check the local id has been been filled with
        // non=zero bytes.
        assert_ne!(router.local_id(), Id::from_u16(0));

        // Check the default constants.
        assert_eq!(router.rt.k, 20);
        assert_eq!(router.rt.max_bucket_size, 20);

        // Check the collections are empty.
        assert!(router.rt.buckets.is_empty());
        assert!(router.rt.peer_list.is_empty());
        assert!(router.rt.id_list.is_empty());
    }

    #[test]
    fn peer_id_not_present() {
        let router = TcpRouter::new(Id::from_u16(0), 1, 20);
        assert!(router.peer_id(localhost_with_port(0)).is_none());
    }

    #[test]
    fn peer_id_is_present() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);

        let id = Id::from_u16(1);
        let addr = localhost_with_port(1);

        assert!(router.insert(id, addr));
        assert_eq!(router.peer_id(addr), Some(id));
    }

    #[test]
    fn insert() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);
        // ... 0001 -> bucket i = 0
        assert!(router.insert(Id::from_u16(1), localhost_with_port(1)));
        // ... 0010 -> bucket i = 1
        assert!(router.insert(Id::from_u16(2), localhost_with_port(2)));
        // ... 0011 -> bucket i = 1
        assert!(router.insert(Id::from_u16(3), localhost_with_port(3)));
    }

    #[test]
    fn insert_defaults() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);

        let id = Id::from_u16(1);
        let addr = localhost_with_port(1);

        assert!(router.insert(id, addr));

        let meta = router.peer_meta(&id).unwrap();
        assert_eq!(meta.listening_addr, addr);
        assert_eq!(meta.conn_addr, None);
        assert_eq!(meta.conn_state, ConnState::Disconnected);
        assert_eq!(meta.last_seen, None);
    }

    #[test]
    fn insert_self() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);
        // Attempt to insert our local id.
        assert!(!router.insert(router.local_id(), localhost_with_port(0)));
    }

    #[test]
    fn insert_duplicate() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);
        // The double insert will still return true.
        assert!(router.insert(Id::from_u16(1), localhost_with_port(1)));
        assert!(router.insert(Id::from_u16(1), localhost_with_port(1)));
    }

    #[test]
    fn insert_duplicate_updates_listening_addr() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);

        let id = Id::from_u16(1);
        let addr = localhost_with_port(1);
        assert!(router.insert(id, addr));
        assert_eq!(router.peer_meta(&id).unwrap().listening_addr, addr);

        // Inserting the same identifier with a different address should result in the new address
        // getting stored.
        let id = Id::from_u16(1);
        let addr = localhost_with_port(2);
        assert!(router.insert(id, addr));
        assert_eq!(router.peer_meta(&id).unwrap().listening_addr, addr);
    }

    #[test]
    fn set_connected() {
        // Set the max bucket size to a low value so we can easily test when it's full.
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);

        // ... 0001 -> bucket i = 0
        let addr = localhost_with_port(1);
        let id = Id::from_u16(1);
        router.insert(id, addr);
        assert!(router.set_connected(id, addr));

        // ... 0010 -> bucket i = 1
        let addr = localhost_with_port(2);
        let id = Id::from_u16(2);
        router.insert(id, addr);
        assert!(router.set_connected(id, addr));

        // ... 0011 -> bucket i = 1
        let addr = localhost_with_port(3);
        let id = Id::from_u16(3);
        router.insert(id, addr);
        assert!(!router.set_connected(id, addr));
    }

    #[test]
    fn set_connected_self() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);
        assert!(!router.set_connected(router.local_id(), localhost_with_port(0)));
    }

    #[test]
    fn set_disconnected() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);
        let id = Id::from_u16(1);
        let addr = localhost_with_port(1);
        assert!(router.insert(id, addr));
        assert!(router.set_connected(id, addr));
        assert!(router.set_disconnected(addr));
    }

    #[test]
    fn set_disconnected_non_existant() {
        let mut router = TcpRouter::new(Id::from_u16(0), 1, 20);
        assert!(!router.set_disconnected(localhost_with_port(0)));
    }

    #[test]
    fn find_k_closest() {
        let mut router = TcpRouter::new(Id::from_u16(0), 5, 20);

        // Generate 5 identifiers and addressses.
        let peers: Vec<(Id, SocketAddr)> = (1..=5)
            .map(|i| (Id::from_u16(i), localhost_with_port(i)))
            .collect();

        for peer in &peers {
            assert!(router.insert(peer.0, peer.1));
            assert!(router.set_connected(peer.0, peer.1));
        }

        let k = 3;
        let k_closest = router.find_k_closest(&router.local_id(), k);

        assert_eq!(k_closest.len(), 3);
        assert!(k_closest.contains(&peers[0]));
        assert!(k_closest.contains(&peers[1]));
        assert!(k_closest.contains(&peers[2]));
    }

    #[test]
    fn find_k_closest_empty() {
        let router = TcpRouter::new(Id::from_u16(0), 5, 20);
        let k = 3;
        let k_closest = router.find_k_closest(&router.local_id(), k);
        assert_eq!(k_closest.len(), 0);
    }

    #[test]
    fn select_broadcast_peers() {
        let mut router = TcpRouter::new(Id::from_u16(0), 5, 20);

        // Generate 5 identifiers and addressses.
        let peers: Vec<(Id, SocketAddr)> = (1..=5)
            .map(|i| (Id::from_u16(i), localhost_with_port(i)))
            .collect();

        for peer in peers {
            // Conn address is listening address (all peers received the connections).
            assert!(router.insert(peer.0, peer.1));
            assert!(router.set_connected(peer.0, peer.1));
        }

        // Find the random addresses in each bucket.

        // If the height is 0, we are the last node in the recursion, don't broadcast.
        let h = 0;
        assert!(router.select_broadcast_peers(h).is_none());

        let h = 1;
        // Should be present.
        let selected_peers = router.select_broadcast_peers(h).unwrap();
        assert_eq!(selected_peers.len(), 1);
        // Height for selected peer should be 0.
        assert_eq!(selected_peers[0].0, 0);

        // TODO: Bucket at index 0 should contain the id corresponding to the address.
    }
}
