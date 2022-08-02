#![cfg(all(feature = "codec", feature = "sync"))]

use std::{
    collections::HashMap,
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

use kadmium::{
    codec::MessageCodec,
    message::{Message, Nonce, Response},
    Id, Kadcast, ProcessData, SyncRoutingTable,
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

    fn process_data(&self, _state: KadNode) {
        let rt = tokio::runtime::Handle::current();

        // The handle should be tracked by the external state's task accounting so it doesn't get
        // dropped accidentally.
        let _handle = rt.spawn(async {
            println!("Running on the async executor, processing message data");
        });
    }
}

#[async_trait::async_trait]
impl Kadcast for KadNode {
    // Shorten the default for testing purposes.
    const PING_INTERVAL_SECS: u64 = 1;

    fn routing_table(&self) -> &SyncRoutingTable {
        &self.routing_table
    }

    async fn unicast(&self, addr: SocketAddr, message: Message) {
        // Track the sent messages to make test assertions easier.
        let id = self.sent_message_counter.fetch_add(1, Ordering::SeqCst);
        self.sent_messages.write().insert(id, message.clone());

        let span = self.node().span().clone();
        info!(parent: span.clone(), "sending {:?}", message);

        let _ = Writing::unicast(self, addr, message).unwrap().await;
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
    pub routing_table: SyncRoutingTable,
    sent_message_counter: Arc<AtomicU64>,
    pub sent_messages: Arc<RwLock<HashMap<u64, Message>>>,
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
            routing_table: SyncRoutingTable::new(id, 20),
            sent_message_counter: Arc::new(AtomicU64::new(0)),
            sent_messages: Arc::new(RwLock::new(HashMap::new())),
            received_messages: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn start_periodic_tasks(&self) {
        // PING/PONG
        self.ping().await;

        // OVERLAY CONSTRUCTION
        self.peer().await;
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

        assert!(self
            .received_messages
            .write()
            .insert(message.nonce(), message.clone())
            .is_none());

        let response =
            self.routing_table
                .process_message::<KadNode, Data>(self.clone(), message, source);

        match response {
            Some(Response::Unicast(message)) => {
                Kadcast::unicast(self, source, message).await;
            }
            Some(Response::Broadcast(broadcast)) => {
                for (addr, message) in broadcast {
                    Kadcast::unicast(self, addr, message).await;
                }
            }
            None => {}
        }

        Ok(())
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
impl Handshake for KadNode {
    async fn perform_handshake(&self, mut conn: Connection) -> io::Result<Connection> {
        let local_id = self.routing_table.local_id();
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
                    .insert(peer_id, peer_addr, Some(conn_addr)));

                // Respond with our local ID and port.
                stream.write_all(&local_id.bytes()).await?;
                stream
                    .write_u16_le(self.node().listening_addr().unwrap().port())
                    .await?;

                // Set the peer as connected.
                assert!(self.routing_table.set_connected(conn_addr));
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

                // If we initiate the connection, we must have space to connect, `can_connect`
                // should have been checked before opening the connection and it will be
                // checked again in `set_connected`. Idem, `insert` shouldn't be needed here,
                // since we should only be initiating connections with peers we know about.
                assert!(self
                    .routing_table
                    .insert(peer_id, peer_addr, Some(peer_addr)));
                assert!(self.routing_table.set_connected(peer_addr));
            }
        }

        Ok(conn)
    }
}

#[async_trait::async_trait]
impl Disconnect for KadNode {
    async fn handle_disconnect(&self, addr: SocketAddr) {
        self.routing_table.set_disconnected(addr);
    }
}
