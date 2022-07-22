#![feature(int_log)]

use kadmium::message::{FindKNodes, Message, Ping};
use pea2pea::{
    protocols::{Handshake, Reading, Writing},
    Pea2Pea,
};
use rand::{thread_rng, Rng};

mod common;
#[allow(unused_imports)]
use crate::common::{enable_tracing, KadNode};

#[tokio::test]
async fn connect_two_nodes() {
    let mut rng = thread_rng();
    let id_a = rng.gen();
    let id_b = rng.gen();
    // Unlikely but just in case we hit this case.
    assert_ne!(id_a, id_b);

    // Start two nodes and have them establish a connection.
    let node_a = KadNode::new(id_a).await;
    node_a.enable_handshake().await;

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;

    // The handshake is enacted here, assertions are checked in the handshake protocol
    // implementation.
    assert!(node_a
        .node()
        .connect(node_b.node().listening_addr().unwrap())
        .await
        .is_ok());
}

#[tokio::test]
async fn ping_pong_two_nodes() {
    // enable_tracing();

    let mut rng = thread_rng();
    let id_a = rng.gen();
    let id_b = rng.gen();
    // Unlikely but just in case we hit this case.
    assert_ne!(id_a, id_b);

    // Start two nodes and have them establish a connection.
    let node_a = KadNode::new(id_a).await;
    node_a.enable_handshake().await;
    node_a.enable_reading().await;
    node_a.enable_writing().await;

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;
    node_b.enable_reading().await;
    node_b.enable_writing().await;

    node_a
        .node()
        .connect(node_b.node().listening_addr().unwrap())
        .await
        .unwrap();

    let nonce = rng.gen();

    assert!(node_a
        .unicast(
            node_b.node().listening_addr().unwrap(),
            Message::Ping(Ping {
                nonce,
                id: node_a.routing_table.read().local_id(),
            }),
        )
        .is_ok());

    // Wait for PING to be received and PONG to come back.
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(node_b.received_messages.read().contains_key(&nonce));
    assert!(node_a.received_messages.read().contains_key(&nonce));
}

#[tokio::test]
async fn k_nodes_two_nodes() {
    // enable_tracing();

    let mut rng = thread_rng();
    let id_a = rng.gen();
    let id_b = rng.gen();
    // Unlikely but just in case we hit this case.
    assert_ne!(id_a, id_b);

    // Start two nodes and have them establish a connection.
    let node_a = KadNode::new(id_a).await;
    node_a.enable_handshake().await;
    node_a.enable_reading().await;
    node_a.enable_writing().await;

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;
    node_b.enable_reading().await;
    node_b.enable_writing().await;

    node_a
        .node()
        .connect(node_b.node().listening_addr().unwrap())
        .await
        .unwrap();

    let nonce = rng.gen();

    assert!(node_a
        .unicast(
            node_b.node().listening_addr().unwrap(),
            Message::FindKNodes(FindKNodes {
                nonce,
                id: node_a.routing_table.read().local_id()
            })
        )
        .is_ok());

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(node_b.received_messages.read().contains_key(&nonce));
    assert!(node_a.received_messages.read().contains_key(&nonce));
}
