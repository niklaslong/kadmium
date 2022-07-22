use std::{
    io,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};

use bytes::BytesMut;
use kadmium::{
    message::{Message, Response},
    router::{Id, RoutingTable},
};
use parking_lot::RwLock;
use pea2pea::{
    protocols::{Handshake, Reading, Writing},
    Config, Connection, ConnectionSide, Node, Pea2Pea,
};
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

#[derive(Clone)]
pub struct KadNode {
    pub node: Node,
    pub routing_table: Arc<RwLock<RoutingTable>>,
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
        MessageCodec {
            codec: LengthDelimitedCodec::new(),
        }
    }

    async fn process_message(&self, source: SocketAddr, message: Self::Message) -> io::Result<()> {
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
        let peer_addr = conn.addr();
        let stream = self.borrow_stream(&mut conn);

        match peer_side {
            // The peer initiated the connection.
            ConnectionSide::Initiator => {
                // Receive the peer's local ID.
                let peer_id = stream.read_u128_le().await?;

                // Scope the lock.
                {
                    let mut rt_g = self.routing_table.write();

                    if rt_g.can_connect(peer_id).0 {
                        debug_assert!(rt_g.insert(peer_id, peer_addr));
                    }
                }

                // Respond with our local ID.
                stream.write_u128_le(local_id).await?;

                // Scope the lock.
                {
                    let mut rt_g = self.routing_table.write();

                    // Set the peer as connected.
                    debug_assert!(rt_g.set_connected(peer_id));
                    rt_g.set_last_seen(peer_id, OffsetDateTime::now_utc());
                }
            }

            // The node initiated the connection.
            ConnectionSide::Responder => {
                // Send our local ID to the peer.
                stream.write_u128_le(local_id).await?;

                // Receive the peer's local ID.
                let peer_id = stream.read_u128_le().await?;

                // Scope the lock.
                {
                    let mut rt_g = self.routing_table.write();

                    // If we initiate the connection, we must have space to connect.
                    debug_assert!(rt_g.can_connect(peer_id).0);
                    rt_g.insert(peer_id, peer_addr);
                    rt_g.set_connected(peer_id);
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
        MessageCodec {
            codec: LengthDelimitedCodec::new(),
        }
    }
}

pub struct MessageCodec {
    codec: LengthDelimitedCodec,
}

impl Decoder for MessageCodec {
    type Item = Message;
    type Error = io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let bytes = match self.codec.decode(src)? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        match bincode::decode_from_slice(&bytes, bincode::config::standard()) {
            Ok((message, _length)) => Ok(Some(message)),
            Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
        }
    }
}

impl Encoder<Message> for MessageCodec {
    type Error = io::Error;

    fn encode(&mut self, message: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        if let Err(e) = bincode::encode_into_slice(message, dst, bincode::config::standard()) {
            return Err(io::Error::new(io::ErrorKind::Other, e));
        }

        Ok(())
    }
}
