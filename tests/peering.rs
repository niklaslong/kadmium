#![cfg(all(feature = "codec", feature = "sync"))]

use std::{sync::atomic::Ordering, time::Duration};

use deadline::deadline;
use kadmium::{Id, Kadcast};
use pea2pea::{
    connect_nodes,
    protocols::{Handshake, Reading, Writing},
    Pea2Pea, Topology,
};
use rand::{thread_rng, Rng};

mod common;
#[allow(unused_imports)]
use crate::common::{enable_tracing, KadNode};

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

    // Start the PING task.
    node_a.ping().await;

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;
    node_b.enable_reading().await;
    node_b.enable_writing().await;

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

    // Create a topology.
    const N: usize = 3;

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

    // Create a new node to bootstrap.
    let id = Id::rand();
    let node = KadNode::new(id).await;
    node.enable_handshake().await;
    node.enable_reading().await;
    node.enable_writing().await;

    // Enable the periodic peer discover task.
    node.peer().await;

    assert!(node
        .node()
        .connect(nodes.first().unwrap().node().listening_addr().unwrap())
        .await
        .is_ok());

    deadline!(Duration::from_secs(3), move || node.node().num_connected()
        == N);
}
