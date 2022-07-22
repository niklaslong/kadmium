use std::io;

use bytes::{Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder, LengthDelimitedCodec};

use crate::message::Message;

pub struct MessageCodec {
    codec: LengthDelimitedCodec,
}

impl MessageCodec {
    pub fn new() -> Self {
        Self {
            codec: LengthDelimitedCodec::new(),
        }
    }
}

impl Default for MessageCodec {
    fn default() -> Self {
        Self::new()
    }
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
        let _ = match bincode::encode_to_vec(message, bincode::config::standard()) {
            Ok(bytes) => self.codec.encode(Bytes::copy_from_slice(&bytes), dst),
            Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::{thread_rng, Rng};

    use super::*;
    use crate::message::Ping;

    #[test]
    fn codec_ping() {
        let mut rng = thread_rng();

        let message = Message::Ping(Ping {
            nonce: rng.gen(),
            id: rng.gen(),
        });

        let mut codec = MessageCodec::new();
        let mut dst = BytesMut::new();

        assert!(codec.encode(message.clone(), &mut dst).is_ok());
        assert_eq!(codec.decode(&mut dst).unwrap().unwrap(), message);
    }
}
