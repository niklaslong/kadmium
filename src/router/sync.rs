use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use parking_lot::RwLock;
use rand::{thread_rng, Rng};
use time::OffsetDateTime;

use crate::{
    id::Id,
    message::{FindKNodes, Message, Nonce, Ping, Response},
    router::{ConnState, RoutingTable},
    traits::ProcessData,
};

/// A routing table implementation suitable for use in async contexts.
///
/// It wraps [`RoutingTable`] and adds [`Nonce`] checking for request/response pairs.
#[cfg_attr(doc_cfg, doc(cfg(feature = "sync")))]
#[derive(Debug, Default, Clone)]
pub struct SyncRoutingTable {
    routing_table: Arc<RwLock<RoutingTable>>,
    sent_nonces: Arc<RwLock<HashMap<Nonce, OffsetDateTime>>>,
}

impl SyncRoutingTable {
    pub fn new(local_id: Id, max_bucket_size: u8, k: u8) -> Self {
        Self {
            routing_table: Arc::new(RwLock::new(RoutingTable::new(local_id, max_bucket_size, k))),
            ..Default::default()
        }
    }

    pub fn local_id(&self) -> Id {
        self.routing_table.read().local_id()
    }

    pub fn insert(&self, id: Id, listening_addr: SocketAddr) -> bool {
        self.routing_table.write().insert(id, listening_addr)
    }

    pub fn set_connected(&self, id: Id, conn_addr: SocketAddr) -> bool {
        self.routing_table.write().set_connected(id, conn_addr)
    }

    pub fn set_disconnected(&self, conn_addr: SocketAddr) -> bool {
        self.routing_table.write().set_disconnected(conn_addr)
    }

    pub fn is_connected(&self, addr: SocketAddr) -> bool {
        let rt_g = self.routing_table.read();
        if let Some(id) = rt_g.peer_id(addr) {
            if let Some(peer_meta) = rt_g.peer_meta(&id) {
                return matches!(peer_meta.conn_state, ConnState::Connected);
            }
        }

        false
    }

    pub fn disconnected_addrs(&self) -> Vec<SocketAddr> {
        self.routing_table
            .read()
            .peer_list
            .iter()
            .filter(|(_, &peer_meta)| matches!(peer_meta.conn_state, ConnState::Disconnected))
            .map(|(_, &peer_meta)| peer_meta.listening_addr)
            .collect()
    }

    pub fn connected_addrs(&self) -> Vec<SocketAddr> {
        self.routing_table
            .read()
            .peer_list
            .iter()
            .filter(|(_, &peer_meta)| matches!(peer_meta.conn_state, ConnState::Connected))
            .map(|(_, &peer_meta)| peer_meta.conn_addr.unwrap())
            .collect()
    }

    pub fn select_search_peers(&self, alpha: usize) -> Vec<(Id, SocketAddr, bool)> {
        let mut ids: Vec<_> = self
            .routing_table
            .read()
            .peer_list
            .iter()
            .map(|(&candidate_id, &candidate_meta)| {
                let (addr, is_connected) = match candidate_meta.conn_state {
                    ConnState::Connected => {
                        // SAFETY: must exist if connected.
                        debug_assert!(candidate_meta.conn_addr.is_some());
                        (candidate_meta.conn_addr.unwrap(), true)
                    }
                    ConnState::Disconnected => (candidate_meta.listening_addr, false),
                };

                (candidate_id, addr, is_connected)
            })
            .collect();

        ids.sort_unstable_by_key(|(candidate_id, _, _)| {
            candidate_id.log2_distance(&self.routing_table.read().local_id())
        });
        ids.truncate(alpha);

        ids
    }

    pub fn select_broadcast_peers(&self, height: u32) -> Option<Vec<(u32, SocketAddr)>> {
        self.routing_table.read().select_broadcast_peers(height)
    }

    pub fn generate_ping(&self) -> Ping {
        let mut rng = thread_rng();
        let nonce = rng.gen();

        self.sent_nonces
            .write()
            .insert(nonce, OffsetDateTime::now_utc());

        Ping {
            nonce,
            id: self.routing_table.read().local_id(),
        }
    }

    pub fn generate_find_k_nodes(&self) -> FindKNodes {
        let mut rng = thread_rng();
        let nonce = rng.gen();

        self.sent_nonces
            .write()
            .insert(nonce, OffsetDateTime::now_utc());

        FindKNodes {
            nonce,
            id: self.routing_table.read().local_id(),
        }
    }

    pub fn process_message<S: Clone, T: ProcessData<S>>(
        &self,
        state: S,
        message: Message,
        source: SocketAddr,
    ) -> Option<Response> {
        if message.is_response() && self.sent_nonces.read().contains_key(&message.nonce()) {
            // TODO: record latency, should there be a separation with PING/PONG?
        }

        self.routing_table
            .write()
            .process_message::<S, T>(state, message, source)
    }
}
