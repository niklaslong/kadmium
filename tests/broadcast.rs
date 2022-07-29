#![cfg(all(feature = "codec", feature = "sync"))]

use kadmium::{Id, Kadcast};
use pea2pea::{
    connect_nodes,
    protocols::{Handshake, Reading, Writing},
    Topology,
};

mod common;
#[allow(unused_imports)]
use crate::common::{enable_tracing, KadNode};

#[tokio::test]
async fn broadcast_full_mesh() {
    // enable_tracing();

    const N: usize = 10;

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
    let nonce = broadcaster.kadcast("Hello, world!".into()).await;

    // This needs to be longer when the test is run with more nodes.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    for node in nodes {
        let received_messages_g = node.received_messages.read();

        assert_eq!(received_messages_g.len(), 1);
        assert!(received_messages_g.contains_key(&nonce));
    }
}
