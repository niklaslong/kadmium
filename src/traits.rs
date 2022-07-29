use std::net::SocketAddr;

use bytes::Bytes;
use rand::{thread_rng, Rng};

use crate::{
    id::Id,
    message::{Chunk, Message, Nonce},
    router::AsyncRoutingTable,
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

// self.ping().await;
// self.mesh().await;
// self.kadcast(block.into()).await;
// self.kadcast(tx.into()).await;

#[async_trait::async_trait]
pub trait Kadcast {
    const PING_INTERVAL_SECS: u16 = 30;

    // TODO: maybe make these depend on min peers?
    const BOOTSTRAP_INTERVAL_SECS: u16 = 10;
    const MESH_INTERVAL_SECS: u16 = 60;

    fn routing_table(&self) -> &AsyncRoutingTable;

    async fn unicast(&self, dst: SocketAddr, message: Message);

    async fn ping(&self) {}

    async fn mesh(&self) {
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
