#![cfg(all(feature = "codec", feature = "sync"))]

use kadmium::Kadcast;
use pea2pea::{connect_nodes, Pea2Pea, Topology};

mod common;
use rand::{seq::SliceRandom, thread_rng};

#[allow(unused_imports)]
use crate::common::{create_n_nodes, enable_tracing, KadNode};

#[tokio::test]
async fn broadcast_full_mesh() {
    // enable_tracing();

    const N: usize = 10;
    let mut nodes = create_n_nodes(N, "hrw").await;

    // If this fails, it may be because the `ulimit` is not high enough.
    assert!(connect_nodes(&nodes, Topology::Mesh).await.is_ok());

    let broadcaster = nodes.pop().unwrap();
    let nonce = broadcaster.kadcast("Hello, world!".into()).await;

    // This needs to be longer when the test is run with more nodes.
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    for node in nodes {
        let received_messages_g = node.received_messages.read();

        assert_eq!(received_messages_g.len(), 1);
        assert!(received_messages_g.contains_key(&nonce));
    }
}

async fn break_connections(nodes: &[KadNode], n: usize) {
    for _ in 0..n {
        // Randomly select two peers to disconnect.
        let mut rng = thread_rng();

        let node = nodes.choose(&mut rng).unwrap();
        let addr = *node.node().connected_addrs().choose(&mut rng).unwrap();
        assert!(node.disconnect(addr).await);
    }
}

#[ignore]
#[tokio::test]
async fn broadcast_partial_mesh() {
    // enable_tracing();

    const N: usize = 10;
    let mut nodes = create_n_nodes(N, "hrwd").await;

    // If this fails, it may be because the `ulimit` is not high enough.
    assert!(connect_nodes(&nodes, Topology::Mesh).await.is_ok());

    // 9900/2 = 4950
    break_connections(&nodes, 4).await;

    let mut ack = 0;
    for node in &nodes {
        ack += node.node().num_connected()
    }

    dbg!(ack / 2);

    let broadcaster = nodes.pop().unwrap();
    let nonce = broadcaster.kadcast("Hello, world!".into()).await;

    // This needs to be longer when the test is run with more nodes.
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let mut received: u8 = 0;

    for node in nodes {
        let received_messages_g = node.received_messages.read();

        if received_messages_g.contains_key(&nonce) {
            received += 1
        }

        //  assert_eq!(received_messages_g.len(), 1);
        //  assert!(received_messages_g.contains_key(&nonce));
    }

    dbg!(received);
}
