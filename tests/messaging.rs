#![cfg(all(feature = "codec", feature = "sync"))]

use std::time::Duration;

use deadline::deadline;
use kadmium::{
    message::{FindKNodes, KNodes, Message, Ping, Pong},
    Id,
};
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
    let id_a = Id::new(rng.gen());
    let id_b = Id::new(rng.gen());
    // Unlikely but just in case we hit this case.
    assert_ne!(id_a, id_b);

    // Start two nodes and have them establish a connection.
    let node_a = KadNode::new(id_a).await;
    node_a.enable_handshake().await;
    node_a.node().start_listening().await.unwrap();

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;
    node_b.node().start_listening().await.unwrap();

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

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;
    node_b.enable_reading().await;
    node_b.enable_writing().await;
    node_b.node().start_listening().await.unwrap();

    node_a
        .node()
        .connect(node_b.node().listening_addr().unwrap())
        .await
        .unwrap();

    let nonce = rng.gen();
    let ping = Message::Ping(Ping {
        nonce,
        id: node_a.router.local_id(),
    });

    let pong = Message::Pong(Pong {
        nonce,
        id: node_b.router.local_id(),
    });

    assert!(node_a
        .unicast(node_b.node().listening_addr().unwrap(), ping.clone())
        .unwrap()
        .await
        .is_ok());

    // Wait for PING to be received and PONG to come back.
    deadline!(Duration::from_millis(100), move || {
        node_b.received_messages.read().get(&nonce) == Some(&ping)
            && node_a.received_messages.read().get(&nonce) == Some(&pong)
    });
}

#[tokio::test]
async fn k_nodes_two_nodes() {
    enable_tracing();

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

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;
    node_b.enable_reading().await;
    node_b.enable_writing().await;
    node_b.node().start_listening().await.unwrap();

    node_a
        .node()
        .connect(node_b.node().listening_addr().unwrap())
        .await
        .unwrap();

    let nonce = rng.gen();
    let find_k_nodes = Message::FindKNodes(FindKNodes {
        nonce,
        id: node_a.router.local_id(),
    });

    let k_nodes = Message::KNodes(KNodes {
        nonce,
        nodes: vec![(
            node_a.router.local_id(),
            node_a.node().listening_addr().unwrap(),
        )],
    });

    assert!(node_a
        .unicast(
            node_b.node().listening_addr().unwrap(),
            find_k_nodes.clone()
        )
        .unwrap()
        .await
        .is_ok());

    deadline!(Duration::from_millis(10), move || {
        node_b.received_messages.read().get(&nonce) == Some(&find_k_nodes)
            && node_a.received_messages.read().get(&nonce) == Some(&k_nodes)
    });
}
