#[cfg(feature = "sync")]
use std::net::SocketAddr;

use bytes::Bytes;
#[cfg(feature = "sync")]
use rand::{seq::SliceRandom, thread_rng, Rng};

#[cfg(feature = "sync")]
use crate::{
    id::Id,
    message::{Chunk, Message, Nonce},
    tcp::SyncRoutingTable,
};

/// A trait used to determine how message-wrapped data is handled.
///
/// Kadmium uses this trait to determine if wrapped data in a [`Chunk`](crate::message::Chunk) message should be propagated
/// further during a broadcast. Valid data will get propagated, invalid data will not.
///
/// Kadmium also uses [`Bytes`] to handle the data in its encoded state, this is so that it can be
/// shipped around easily with the [`MessageCodec`](crate::codec::MessageCodec).
pub trait ProcessData<S>: From<Bytes> {
    /// Returns whether the data is valid or not; the provided implementation returns `true`.
    fn verify_data(&self, _state: S) -> bool {
        true
    }

    /// Processes the data; the provided implementation is a no-op.
    ///
    /// Kadmium doesn't make any assumptions about how this function should be implemented. E.g. if
    /// you want to run it asynchronously, you can pass in a handle to the runtime in the state and
    /// spawn a task; if you want to send it elsewhere for further processing, you could pass in a
    /// channel sender.
    fn process_data(&self, _state: S) {}
}

/// A trait used to enable core kadcast functionality on the implementor.
#[cfg(feature = "sync")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "sync")))]
#[async_trait::async_trait]
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

    /// Returns a clonable reference to the routing table.
    fn routing_table(&self) -> &SyncRoutingTable;

    /// Returns `true` if the address is connected, `false` if it isn't.
    async fn is_connected(&self, addr: SocketAddr) -> bool;

    /// Connects to the address and returns if it was succesful or not.
    ///
    /// Note: Kadmium assumes this method calls [`SyncRoutingTable::insert`] and
    /// [`SyncRoutingTable::set_connected`] appropriately.
    async fn connect(&self, addr: SocketAddr) -> bool;

    /// Disconnects the address and returns `true` if it was connected, returns `false` if it wasn't.
    ///
    /// Note: Kadmium assumes this method calls [`SyncRoutingTable::set_disconnected`] appropriately.
    async fn disconnect(&self, addr: SocketAddr) -> bool;

    /// Sends a message to the destination address.
    async fn unicast(&self, dst: SocketAddr, message: Message);

    /// Starts the periodic ping task.
    async fn ping(&self) {
        let self_clone = self.clone();

        tokio::spawn(async move {
            loop {
                for addr in self_clone.routing_table().connected_addrs() {
                    self_clone
                        .unicast(
                            addr,
                            Message::Ping(self_clone.routing_table().generate_ping()),
                        )
                        .await
                }

                tokio::time::sleep(std::time::Duration::from_secs(Self::PING_INTERVAL_SECS)).await
            }
        });

        // TODO: consider returning the task handle, or at least track it internally.
    }

    /// Starts the periodic peer discovery task.
    async fn peer(&self) {
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
                for (_id, addr, is_connected) in self_clone
                    .routing_table()
                    .select_search_peers(Self::ALPHA.into())
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
                                    self_clone.routing_table().generate_find_k_nodes(),
                                ),
                            )
                            .await;
                    }
                }

                let peer_deficit = Self::PEER_TARGET as i128
                    - self_clone.routing_table().connected_addrs().len() as i128;

                if peer_deficit < 0 {
                    let addrs: Vec<SocketAddr> = {
                        let mut rng = rand::thread_rng();

                        self_clone
                            .routing_table()
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
                            .routing_table()
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
                        if self_clone.routing_table().connected_addrs().len()
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

    /// Broadcast data to the network, following the kadcast protocol.
    async fn kadcast(&self, data: Bytes) -> Nonce {
        let peers = self
            .routing_table()
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
