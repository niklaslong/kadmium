use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use parking_lot::RwLock;
use rand::{thread_rng, Rng};
use time::OffsetDateTime;

use crate::{
    core::{
        id::Id,
        message::{FindKNodes, Message, Nonce, Ping, Response},
        traits::ProcessData,
    },
    tcp::{ConnState, TcpRouter},
};

/// A router implementation suitable for use in async contexts.
///
/// It wraps [`TcpRouter`] and adds [`Nonce`] checking for request/response pairs.
#[cfg_attr(doc_cfg, doc(cfg(feature = "sync")))]
#[derive(Debug, Default, Clone)]
pub struct SyncTcpRouter {
    router: Arc<RwLock<TcpRouter>>,
    sent_nonces: Arc<RwLock<HashMap<Nonce, OffsetDateTime>>>,
}

impl SyncTcpRouter {
    pub fn new(local_id: Id, max_bucket_size: u8, k: u8) -> Self {
        Self {
            router: Arc::new(RwLock::new(TcpRouter::new(local_id, max_bucket_size, k))),
            ..Default::default()
        }
    }

    pub fn local_id(&self) -> Id {
        self.router.read().local_id()
    }

    pub fn insert(&self, id: Id, listening_addr: SocketAddr) -> bool {
        self.router.write().insert(id, listening_addr)
    }

    pub fn set_connected(&self, id: Id, conn_addr: SocketAddr) -> bool {
        self.router.write().set_connected(id, conn_addr)
    }

    pub fn set_disconnected(&self, conn_addr: SocketAddr) -> bool {
        self.router.write().set_disconnected(conn_addr)
    }

    pub fn is_connected(&self, addr: SocketAddr) -> bool {
        self.router.read().is_connected(addr)
    }

    pub fn disconnected_addrs(&self) -> Vec<SocketAddr> {
        self.router.read().disconnected_addrs()
    }

    pub fn connected_addrs(&self) -> Vec<SocketAddr> {
        self.router.read().connected_addrs()
    }

    pub fn select_search_peers(&self, alpha: usize) -> Vec<(Id, SocketAddr, bool)> {
        let mut ids: Vec<_> = self
            .router
            .read()
            .routing_table()
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
            candidate_id.log2_distance(&self.router.read().local_id())
        });
        ids.truncate(alpha);

        ids
    }

    pub fn select_broadcast_peers(&self, height: u32) -> Option<Vec<(u32, SocketAddr)>> {
        self.router.read().select_broadcast_peers(height)
    }

    pub fn generate_ping(&self) -> Ping {
        let mut rng = thread_rng();
        let nonce = rng.gen();

        self.sent_nonces
            .write()
            .insert(nonce, OffsetDateTime::now_utc());

        Ping {
            nonce,
            id: self.router.read().local_id(),
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
            // TODO: the local id should be used when bootstrapping but later it should choose a
            // id at random in a bucket that hasn't seen much activity lately. In practice we could
            // just shoot one out periodcially for each populated bucket.
            id: self.router.read().local_id(),
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

        self.router
            .write()
            .process_message::<S, T>(state, message, source)
    }
}
