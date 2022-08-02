#![cfg(all(feature = "codec", feature = "sync"))]

use kadmium::{Id, Kadcast};
use pea2pea::{
    protocols::{Handshake, Reading, Writing},
    Pea2Pea,
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

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    // Freeze state for assertions.
    let a_sent_g = node_a.sent_messages.read();
    let a_received_g = node_a.received_messages.read();

    let b_sent_g = node_b.sent_messages.read();
    let b_received_g = node_b.received_messages.read();

    // Two messages should have been sent in the interval of time.
    const N: usize = 2;
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
