#[cfg(feature = "sync")]
use std::net::SocketAddr;

use bytes::Bytes;
#[cfg(feature = "sync")]
use rand::{thread_rng, Rng};

#[cfg(feature = "sync")]
use crate::{
    id::Id,
    message::{Chunk, Message, Nonce},
    router::sync::SyncRoutingTable,
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
pub trait Kadcast {
    /// The interval between periodic pings in seconds.
    const PING_INTERVAL_SECS: u16 = 30;
    /// The interval between periodic requests for peers while below the mininum number of peers.
    const BOOTSTRAP_INTERVAL_SECS: u16 = 10;
    /// The interval between periodic requests for peers while above the minimum number of peers.
    const DISCOVERY_INTERVAL_SECS: u16 = 60;

    /// Returns a clonable reference to the routing table.
    fn routing_table(&self) -> &SyncRoutingTable;

    /// Sends a message to the destination address.
    async fn unicast(&self, dst: SocketAddr, message: Message);

    /// Starts the periodic ping task.
    async fn ping(&self) {}

    /// Starts the periodic peer discovery task.
    async fn peer(&self) {
        // tokio::spawn(async move || loop {
        //     let rt_g = self.routing_table.read();
        //     // Continually mesh, if the peer count is less than the min.
        //     let sleep_duration =
        //         std::time::Duration::from_secs(if rt.peer_count() < rt.min_peers() {
        //             BOOTSTRAP_INTERVAL_SECS
        //         } else {
        //             MESH_INTERVAL_SECS
        //         });

        //     tokio::time::sleep(sleep_duration).await;
        // })
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
