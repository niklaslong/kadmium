#![feature(int_log)]

use pea2pea::{protocols::Handshake, Pea2Pea};
use rand::{thread_rng, Rng};

mod common;
use crate::common::KadNode;

#[tokio::test]
async fn connect_two_nodes() {
    let mut rng = thread_rng();
    let id_a = rng.gen();
    let id_b = rng.gen();
    // Unlikely but just in case we hit this case.
    assert_ne!(id_a, id_b);

    // Compute the distance and the bucket index.
    let distance: u128 = id_a ^ id_b;
    let i = distance.log2();

    // Start two nodes and have them establish a connection.
    let node_a = KadNode::new(id_a).await;
    node_a.enable_handshake().await;

    let node_b = KadNode::new(id_b).await;
    node_b.enable_handshake().await;

    node_a
        .node()
        .connect(node_b.node().listening_addr().unwrap())
        .await
        .unwrap();

    // The buckets of both nodes should contain the other's peer ID at index 0, since the XOR
    // distance between the two IDs is 1 and log2(1) is 0.
    assert!(node_a
        .routing_table
        .read()
        .buckets()
        .get(&i)
        .unwrap()
        .contains(&node_b.routing_table.read().local_id()));

    assert!(node_b
        .routing_table
        .read()
        .buckets()
        .get(&i)
        .unwrap()
        .contains(&node_a.routing_table.read().local_id()));
}
