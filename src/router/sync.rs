use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use parking_lot::RwLock;
use rand::{thread_rng, Rng};
use time::OffsetDateTime;

use crate::{
    id::Id,
    message::{Message, Nonce, Ping, Response},
    router::RoutingTable,
    traits::ProcessData,
};

#[cfg_attr(doc_cfg, doc(cfg(feature = "sync")))]
#[derive(Debug, Default, Clone)]
/// A routing table implementation suitable for use in async contexts.
///
/// It wraps [`RoutingTable`] and adds [`Nonce`] checking for request/response pairs.
pub struct SyncRoutingTable {
    routing_table: Arc<RwLock<RoutingTable>>,
    sent_nonces: Arc<RwLock<HashMap<Nonce, OffsetDateTime>>>,
}

impl SyncRoutingTable {
    pub fn new(local_id: Id, max_bucket_size: u8) -> Self {
        Self {
            routing_table: Arc::new(RwLock::new(RoutingTable::new(local_id, max_bucket_size))),
            ..Default::default()
        }
    }

    pub fn local_id(&self) -> Id {
        self.routing_table.read().local_id()
    }

    pub fn insert(
        &self,
        id: Id,
        listening_addr: SocketAddr,
        conn_addr: Option<SocketAddr>,
    ) -> bool {
        self.routing_table
            .write()
            .insert(id, listening_addr, conn_addr)
    }

    pub fn set_connected(&self, conn_addr: SocketAddr) -> bool {
        self.routing_table.write().set_connected(conn_addr)
    }

    pub fn set_disconnected(&self, conn_addr: SocketAddr) {
        self.routing_table.write().set_disconnected(conn_addr)
    }

    pub fn connected_addrs(&self) -> Vec<SocketAddr> {
        // Easiest collection to access instead of iterating over the buckets or the entire peer
        // list containing both connected and disconnected addrs.
        self.routing_table.read().id_list.keys().copied().collect()
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
