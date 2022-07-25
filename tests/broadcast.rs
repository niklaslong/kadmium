#![cfg(feature = "codec")]

use bytes::Bytes;
use kadmium::{
    message::{Chunk, Message},
    Id,
};
use pea2pea::{
    connect_nodes,
    protocols::{Handshake, Reading, Writing},
    Topology,
};
use rand::{thread_rng, Rng};

mod common;
#[allow(unused_imports)]
use crate::common::{enable_tracing, KadNode};

#[tokio::test]
async fn broadcast_full_mesh() {
    // enable_tracing();

    const N: usize = 10;
    let mut rng = thread_rng();

    let mut nodes = Vec::with_capacity(N);
    for _ in 0..N {
        let id = Id::rand();
        let node = KadNode::new(id).await;
        node.enable_handshake().await;
        node.enable_reading().await;
        node.enable_writing().await;

        nodes.push(node);
    }

    // If this fails, it may be because the `ulimit` is not high enough.
    assert!(connect_nodes(&nodes, Topology::Mesh).await.is_ok());

    let broadcaster = nodes.pop().unwrap();
    let peers = broadcaster
        .routing_table
        .read()
        .select_broadcast_peers(Id::BITS as u32)
        .unwrap();

    let nonce = rng.gen();

    for (height, addr) in peers {
        let message = Message::Chunk(Chunk {
            // Can be used to trace the broadcast. If set differently for each peer here, it will
            // be the same within a propagation sub-tree.
            nonce,
            height,
            data: Bytes::from("Hello, world!"),
        });

        assert!(broadcaster.unicast(addr, message).unwrap().await.is_ok());
    }

    // This needs to be longer when the test is run with more nodes.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    for node in nodes {
        let received_messages_g = node.received_messages.read();

        assert_eq!(received_messages_g.len(), 1);
        assert!(received_messages_g.contains_key(&nonce));
    }
}
