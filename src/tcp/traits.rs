#[cfg(feature = "sync")]
use std::{future::Future, net::SocketAddr};

use bytes::Bytes;
#[cfg(feature = "sync")]
use rand::{seq::SliceRandom, thread_rng, Rng};

#[cfg(feature = "sync")]
use crate::{
    core::id::Id,
    core::message::{Chunk, Message, Nonce},
    tcp::SyncTcpRouter,
};

/// A trait used to enable core kadcast functionality on the implementor.
#[cfg(feature = "sync")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "sync")))]
pub trait Kadcast
where
    Self: Clone + Send + Sync + 'static,
{
    /// The number of nodes to query for peers at each search.
    const ALPHA: u16 = 3;
    /// The peer count target for this node.
    const PEER_TARGET: u16 = 10;
    /// The interval between periodic pings in seconds.
    const PING_INTERVAL_SECS: u64 = 30;
    /// The interval between periodic requests for peers while below the mininum number of peers.
    const BOOTSTRAP_INTERVAL_SECS: u64 = 10;
    /// The interval between periodic requests for peers while above the minimum number of peers.
    const DISCOVERY_INTERVAL_SECS: u64 = 60;

    /// Returns a clonable reference to the router.
    fn router(&self) -> &SyncTcpRouter;

    /// Returns `true` if the address is connected, `false` if it isn't.
    fn is_connected(&self, addr: SocketAddr) -> impl Future<Output = bool> + Send;

    /// Connects to the address and returns if it was succesful or not.
    ///
    /// Note: Kadmium assumes this method calls [`SyncTcpRouter::insert`] and
    /// [`SyncTcpRouter::set_connected`] appropriately.
    fn connect(&self, addr: SocketAddr) -> impl Future<Output = bool> + Send;

    /// Disconnects the address and returns `true` if it was connected, returns `false` if it wasn't.
    ///
    /// Note: Kadmium assumes this method calls [`SyncTcpRouter::set_disconnected`] appropriately.
    fn disconnect(&self, addr: SocketAddr) -> impl Future<Output = bool> + Send;

    /// Sends a message to the destination address.
    fn unicast(&self, dst: SocketAddr, message: Message) -> impl Future<Output = ()> + Send;

    /// Starts the periodic ping task.

    fn ping(&self) -> impl Future<Output = ()> + Send {
        async {
            let self_clone = self.clone();

            tokio::spawn(async move {
                loop {
                    for addr in self_clone.router().connected_addrs() {
                        self_clone
                            .unicast(addr, Message::Ping(self_clone.router().generate_ping()))
                            .await
                    }

                    tokio::time::sleep(std::time::Duration::from_secs(Self::PING_INTERVAL_SECS))
                        .await
                }
            });

            // TODO: consider returning the task handle, or at least track it internally.
        }
    }

    /// Starts the periodic peer discovery task.
    fn peer(&self) -> impl Future<Output = ()> + Send {
        async {
            // TODO: a few current issues to consider:
            //
            // 1. identifiers are more likely to be in higher index buckets, not necessarily an issue
            //    so long as bucket size is above the minimum number of peers.
            // 2. the above also guaranties a search returning K nodes can indeed return K nodes, so
            //    long as K is below the minimum number of peers. If K is larger a node will return at
            //    worst min(min peers, K) and at best min(peers, K).
            //
            // Therefore: bucket size >= min peers >= K is likely ideal.

            let self_clone = self.clone();

            tokio::spawn(async move {
                loop {
                    for (_id, addr, is_connected) in
                        self_clone.router().select_search_peers(Self::ALPHA.into())
                    {
                        let is_connected = match is_connected {
                            true => self_clone.is_connected(addr).await,
                            false => self_clone.connect(addr).await,
                        };

                        if is_connected {
                            self_clone
                                .unicast(
                                    addr,
                                    Message::FindKNodes(
                                        self_clone.router().generate_find_k_nodes(),
                                    ),
                                )
                                .await;
                        }
                    }

                    let peer_deficit = Self::PEER_TARGET as i128
                        - self_clone.router().connected_addrs().len() as i128;

                    if peer_deficit < 0 {
                        let addrs: Vec<SocketAddr> = {
                            let mut rng = rand::thread_rng();

                            self_clone
                                .router()
                                .connected_addrs()
                                .choose_multiple(&mut rng, peer_deficit.unsigned_abs() as usize)
                                .copied()
                                .collect()
                        };

                        for addr in addrs {
                            self_clone.disconnect(addr).await;
                        }
                    }

                    if peer_deficit > 0 {
                        let addrs: Vec<SocketAddr> = {
                            let mut rng = rand::thread_rng();
                            self_clone
                                .router()
                                .disconnected_addrs()
                                .choose_multiple(&mut rng, peer_deficit as usize)
                                .copied()
                                .collect()
                        };

                        for addr in addrs {
                            self_clone.connect(addr).await;
                        }
                    }

                    // Check the peer counts again.
                    let sleep_duration = {
                        std::time::Duration::from_secs(
                            if self_clone.router().connected_addrs().len()
                                < Self::PEER_TARGET.into()
                            {
                                Self::BOOTSTRAP_INTERVAL_SECS
                            } else {
                                Self::DISCOVERY_INTERVAL_SECS
                            },
                        )
                    };

                    tokio::time::sleep(sleep_duration).await;
                }
            });
        }
    }

    // TODO: work out how and if data should be chunked (1 block per-message or multiple smaller
    // messages). Up to the caller for now.
    /// Broadcast data to the network, following the kadcast protocol.
    fn kadcast(&self, data: Bytes) -> impl Future<Output = Nonce> + Send {
        async move {
            let peers = self
                .router()
                .select_broadcast_peers(Id::BITS as u32)
                .unwrap();

            // TODO: record nonce somewhere.
            let nonce = {
                let mut rng = thread_rng();
                rng.gen()
            };

            for (height, addr) in peers {
                let message = Message::Chunk(Chunk {
                    // Can be used to trace the broadcast. If set differently for each peer here, it will
                    // be the same within a propagation sub-tree.
                    nonce,
                    height,
                    // Cheap as the backing storage is shared amongst instances.
                    data: data.clone(),
                });

                self.unicast(addr, message).await;
            }

            nonce
        }
    }
}
