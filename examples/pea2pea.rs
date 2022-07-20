use std::{io, net::SocketAddr, sync::Arc};

use bytes::BytesMut;
use kadcast::{
    message::{Message, Response},
    tree::RoutingTable,
};
use parking_lot::RwLock;
use pea2pea::{
    protocols::{Reading, Writing},
    ConnectionSide, Node as PNode, Pea2Pea,
};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

fn main() {}

#[derive(Clone)]
struct Node {
    pnode: PNode,
    routing_table: Arc<RwLock<RoutingTable>>,
}

impl Node {}

impl Pea2Pea for Node {
    fn node(&self) -> &PNode {
        &self.pnode
    }
}

#[async_trait::async_trait]
impl Reading for Node {
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

impl Writing for Node {
    type Message = Message;
    type Codec = MessageCodec;

    fn codec(&self, _addr: SocketAddr, _side: ConnectionSide) -> Self::Codec {
        MessageCodec {
            codec: LengthDelimitedCodec::new(),
        }
    }
}

struct MessageCodec {
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
