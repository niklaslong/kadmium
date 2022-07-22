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
    router::{Id, RoutingTable},
};
use parking_lot::RwLock;
use pea2pea::{
    protocols::{Handshake, Reading, Writing},
    Config, Connection, ConnectionSide, Node, Pea2Pea,
};
use time::OffsetDateTime;
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
                ..Default::default()
            })
            .await
            .unwrap(),
            routing_table: Arc::new(RwLock::new(RoutingTable::new(id, 20))),
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
        debug!(parent: span.clone(), "processing {:?}", message);

        if let Some(nonce) = message.nonce() {
            assert!(self
                .received_messages
                .write()
                .insert(nonce, message.clone())
                .is_none())
        }

        // Scope the lock.
        let response = self.routing_table.write().process_message(message);

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
        let mut peer_addr = conn.addr();
        let stream = self.borrow_stream(&mut conn);

        match peer_side {
            // The peer initiated the connection.
            ConnectionSide::Initiator => {
                // Receive the peer's local ID.
                let peer_id = stream.read_u128_le().await?;
                let peer_port = stream.read_u16_le().await?;

                // Ports will be different since connection was opened by the peer (a new stream is
                // created per connection).
                assert_ne!(peer_addr.port(), peer_port);
                peer_addr.set_port(peer_port);

                // Scope the lock.
                {
                    let mut rt_g = self.routing_table.write();

                    if rt_g.can_connect(peer_id).0 {
                        assert!(rt_g.insert(peer_id, peer_addr));
                    }
                }

                // Respond with our local ID and port.
                stream.write_u128_le(local_id).await?;
                stream
                    .write_u16_le(self.node().listening_addr().unwrap().port())
                    .await?;

                // Scope the lock.
                {
                    let mut rt_g = self.routing_table.write();

                    // Set the peer as connected.
                    assert!(rt_g.set_connected(peer_id));
                    rt_g.set_last_seen(peer_id, OffsetDateTime::now_utc());
                }
            }

            // The node initiated the connection.
            ConnectionSide::Responder => {
                // Send our local ID to the peer.
                stream.write_u128_le(local_id).await?;
                stream
                    .write_u16_le(self.node().listening_addr().unwrap().port())
                    .await?;

                // Receive the peer's local ID and port.
                let peer_id = stream.read_u128_le().await?;
                let peer_port = stream.read_u16_le().await?;

                // Ports should be the same, since we initiated the connection to the peer's
                // listener.
                assert_eq!(peer_addr.port(), peer_port);
                peer_addr.set_port(peer_port);

                // Scope the lock.
                {
                    let mut rt_g = self.routing_table.write();

                    // If we initiate the connection, we must have space to connect.
                    assert!(rt_g.can_connect(peer_id).0);
                    assert!(rt_g.insert(peer_id, peer_addr));
                    assert!(rt_g.set_connected(peer_id));
                    rt_g.set_last_seen(peer_id, OffsetDateTime::now_utc());
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

// pub struct MessageCodec {
//     codec: LengthDelimitedCodec,
// }
//
// impl MessageCodec {
//     fn new() -> Self {
//         Self {
//             codec: LengthDelimitedCodec::new(),
//         }
//     }
// }
//
// impl Decoder for MessageCodec {
//     type Item = Message;
//     type Error = io::Error;
//
//     fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
//         let bytes = match self.codec.decode(src)? {
//             Some(bytes) => bytes,
//             None => return Ok(None),
//         };
//
//         match bincode::decode_from_slice(&bytes, bincode::config::standard()) {
//             Ok((message, _length)) => Ok(Some(message)),
//             Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
//         }
//     }
// }
//
// impl Encoder<Message> for MessageCodec {
//     type Error = io::Error;
//
//     fn encode(&mut self, message: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
//         let _ = match bincode::encode_to_vec(message, bincode::config::standard()) {
//             Ok(bytes) => self.codec.encode(Bytes::copy_from_slice(&bytes), dst),
//             Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
//         };
//
//         Ok(())
//     }
// }
//
// #[test]
// fn codec_ping() {
//     use kadmium::message::{Message, Ping};
//     use rand::{thread_rng, Rng};
//
//     let mut rng = thread_rng();
//
//     let message = Message::Ping(Ping {
//         nonce: rng.gen(),
//         id: rng.gen(),
//     });
//
//     let mut codec = MessageCodec::new();
//     let mut dst = BytesMut::new();
//
//     assert!(codec.encode(message.clone(), &mut dst).is_ok());
//     assert_eq!(codec.decode(&mut dst).unwrap().unwrap(), message);
// }
