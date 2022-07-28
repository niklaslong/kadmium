#![cfg(feature = "codec")]

use std::{
    collections::HashMap,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use kadmium::{
    codec::MessageCodec,
    message::{Message, Nonce, Response},
    Id, ProcessData, RoutingTable,
};
use parking_lot::RwLock;
use pea2pea::{
    protocols::{Disconnect, Handshake, Reading, Writing},
    Config, Connection, ConnectionSide, Node, Pea2Pea,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::*;
use tracing_subscriber::{fmt, EnvFilter};

#[allow(dead_code)]
pub fn enable_tracing() {
    fmt()
        .with_test_writer()
        .with_env_filter(EnvFilter::from_default_env())
        .init();
}

use bytes::Bytes;

struct Data(String);

impl ProcessData<KadNode> for Data {
    fn verify_data(&self, _state: KadNode) -> bool {
        matches!(self, Data(data) if data == "Hello, world!")
    }
}

impl From<Bytes> for Data {
    fn from(bytes: Bytes) -> Self {
        Data(String::from_utf8(bytes.to_vec()).unwrap())
    }
}

#[derive(Clone)]
pub struct KadNode {
    pub node: Node,
    pub routing_table: Arc<RwLock<RoutingTable>>,
    pub received_messages: Arc<RwLock<HashMap<Nonce, Message>>>,
}

impl KadNode {
    pub async fn new(id: Id) -> Self {
        Self {
            node: Node::new(Config {
                listener_ip: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)),
                max_connections: 1024,
                ..Default::default()
            })
            .await
            .unwrap(),
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id, 255))),
            received_messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Pea2Pea for KadNode {
    fn node(&self) -> &Node {
        &self.node
    }
}

#[async_trait::async_trait]
impl Reading for KadNode {
    type Message = Message;
    type Codec = MessageCodec;

    fn codec(&self, _addr: SocketAddr, _side: ConnectionSide) -> Self::Codec {
        MessageCodec::new()
    }

    async fn process_message(&self, source: SocketAddr, message: Self::Message) -> io::Result<()> {
        let span = self.node().span().clone();
        info!(parent: span.clone(), "processing {:?}", message);

        if let Some(nonce) = message.nonce() {
            assert!(self
                .received_messages
                .write()
                .insert(nonce, message.clone())
                .is_none())
        }

        // Scope the locks.
        let response = {
            let mut rt_g = self.routing_table.write();
            rt_g.process_message::<KadNode, Data>(self.clone(), message, source)
        };

        match response {
            Some(Response::Unicast(message)) => {
                let _ = self.unicast(source, message).unwrap().await;
            }
            Some(Response::Broadcast(broadcast)) => {
                for (addr, message) in broadcast {
                    let _ = self.unicast(addr, message).unwrap().await;
                }
            }
            None => {}
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl Handshake for KadNode {
    async fn perform_handshake(&self, mut conn: Connection) -> io::Result<Connection> {
        let local_id = self.routing_table.read().local_id();
        let peer_side = conn.side();
        let conn_addr = conn.addr();
        let mut peer_addr = conn.addr();
        let stream = self.borrow_stream(&mut conn);

        match peer_side {
            // The peer initiated the connection.
            ConnectionSide::Initiator => {
                // Receive the peer's local ID.
                let mut bytes = [0u8; 32];
                stream.read_exact(&mut bytes).await?;
                let peer_id = Id::new(bytes);

                // Ports will be different since connection was opened by the peer (a new stream is
                // created per connection).
                let peer_port = stream.read_u16_le().await?;
                assert_ne!(peer_addr.port(), peer_port);
                peer_addr.set_port(peer_port);

                // Insert the peer.
                assert!(self
                    .routing_table
                    .write()
                    .insert(peer_id, peer_addr, Some(conn_addr)));

                // Respond with our local ID and port.
                stream.write_all(&local_id.bytes()).await?;
                stream
                    .write_u16_le(self.node().listening_addr().unwrap().port())
                    .await?;

                // Set the peer as connected.
                assert!(self.routing_table.write().set_connected(conn_addr));
            }

            // The node initiated the connection.
            ConnectionSide::Responder => {
                // Send our local ID to the peer.
                stream.write_all(&local_id.bytes()).await?;
                stream
                    .write_u16_le(self.node().listening_addr().unwrap().port())
                    .await?;

                // Receive the peer's local ID.
                let mut bytes = [0u8; 32];
                stream.read_exact(&mut bytes).await?;
                let peer_id = Id::new(bytes);

                // Ports should be the same, since we initiated the connection to the peer's
                // listener.
                let peer_port = stream.read_u16_le().await?;
                assert_eq!(peer_addr.port(), peer_port);

                // Scope the lock.
                {
                    let mut rt_g = self.routing_table.write();

                    // If we initiate the connection, we must have space to connect, `can_connect`
                    // should have been checked before opening the connection and it will be
                    // checked again in `set_connected`.
                    assert!(rt_g.insert(peer_id, peer_addr, Some(peer_addr)));
                    assert!(rt_g.set_connected(peer_addr));
                }
            }
        }

        Ok(conn)
    }
}

impl Writing for KadNode {
    type Message = Message;
    type Codec = MessageCodec;

    fn codec(&self, _addr: SocketAddr, _side: ConnectionSide) -> Self::Codec {
        MessageCodec::new()
    }
}

#[async_trait::async_trait]
impl Disconnect for KadNode {
    async fn handle_disconnect(&self, addr: SocketAddr) {
        self.routing_table.write().set_disconnected(addr);
    }
}
