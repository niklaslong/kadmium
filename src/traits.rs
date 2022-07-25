use bytes::Bytes;

/// A trait used to determine if data is valid or not.
///
/// Kadmium uses this trait to determine if wrapped data in a [`Chunk`](crate::message::Chunk) message should be propagated
/// further during a broadcast. Valid data will get propagated, invalid data will not.
///
/// Kadmium also uses [`Bytes`] to handle the data in its encoded state, this is so that it can be
/// shipped around easily with the [`MessageCodec`](crate::codec::MessageCodec).
pub trait VerifyData<S>: From<Bytes> {
    /// Returns whether the data is valid or not.
    fn verify_data(&self, state: S) -> bool;
}
