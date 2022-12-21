#![cfg(all(feature = "codec", feature = "sync"))]

use std::{sync::atomic::Ordering, time::Duration};

use deadline::deadline;
use kadmium::{tcp::Kadcast, Id};
use pea2pea::{
    connect_nodes,
    protocols::{Handshake, Reading, Writing},
    Pea2Pea, Topology,
};
use rand::{thread_rng, Rng};

mod common;
#[allow(unused_imports)]
use crate::common::{create_n_nodes, enable_tracing, KadNode};

#[tokio::test]
async fn periodic_ping_pong() {
    // enable_tracing();

    let mut rng = thread_rng();
    let id_a = Id::new(rng.gen());
    let id_b = Id::new(rng.gen());
    // Unlikely but just in case we hit this case.
    assert_ne!(id_a, id_b);

    // Start two nodes and have them establish a connection.
    let node_a = KadNode::new(id_a).await;
    node_a.enable_handshake().await;
    node_a.enable_reading().await;
    node_a.enable_writing().await;
    node_a.node().start_listening().await.unwrap();

    // Start the PING task.
    node_a.ping().await;

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;
    node_b.enable_reading().await;
    node_b.enable_writing().await;
    node_b.node().start_listening().await.unwrap();

    // The handshake is enacted here, assertions are checked in the handshake protocol
    // implementation.
    assert!(node_a
        .node()
        .connect(node_b.node().listening_addr().unwrap())
        .await
        .is_ok());

    // Wait until N responses (PONGs) have been received back.
    const N: usize = 2;
    deadline!(std::time::Duration::from_secs(3), move || node_a
        .received_message_counter
        .load(Ordering::Relaxed)
        == N as u64);

    // Freeze state for assertions.
    let a_sent_g = node_a.sent_messages.read();
    let a_received_g = node_a.received_messages.read();

    let b_sent_g = node_b.sent_messages.read();
    let b_received_g = node_b.received_messages.read();

    // Two messages should have been sent in the interval of time.
    assert_eq!(a_sent_g.len(), N);
    assert_eq!(b_received_g.len(), N);

    for i in 0..N as u64 {
        // Check PINGs (A -> B).
        let (n, sent_message) = a_sent_g.get_key_value(&i).unwrap();
        let (_nonce, received) = b_received_g.get_key_value(&sent_message.nonce()).unwrap();

        assert_eq!(*n, i);
        assert_eq!(received, sent_message);

        // Check PONGs (B -> A).
        let (n, sent_message) = b_sent_g.get_key_value(&i).unwrap();
        let (_nonce, received) = a_received_g.get_key_value(&sent_message.nonce()).unwrap();

        assert_eq!(*n, i);
        assert_eq!(received, sent_message);
    }
}

#[tokio::test]
async fn bootstrap_peering() {
    // enable_tracing();

    // Create a bunch of nodes...
    const N: usize = 11;
    let mut nodes = create_n_nodes(N, "hrw").await;

    // ...with one extra node to use as a new node in the network.
    let node = nodes.pop().unwrap();
    // Enable the periodic peer discover task.
    node.peer().await;

    // Create the topology (N - 1 nodes), if this fails, it may be because the `ulimit` is not high
    // enough.
    assert!(connect_nodes(&nodes, Topology::Mesh).await.is_ok());

    // Connect the new node into the network.
    assert!(node
        .node()
        .connect(nodes.first().unwrap().node().listening_addr().unwrap())
        .await
        .is_ok());

    // As long as the node is under its min peers, it should keep connecting.
    deadline!(Duration::from_secs(5), move || node.node().num_connected()
        == N - 1);
}
