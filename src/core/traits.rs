use bytes::Bytes;

/// A trait used to determine how message-wrapped data is handled.
///
/// Kadmium uses this trait to determine if wrapped data in a [`Chunk`](crate::message::Chunk) message should be propagated
/// further during a broadcast. Valid data will get propagated, invalid data will not.
///
/// Kadmium also uses [`Bytes`] to handle the data in its encoded state, this is so that it can be
/// shipped around easily with the [`MessageCodec`](crate::codec::MessageCodec).
pub trait ProcessData<S>: TryFrom<Bytes> {
    /// Returns whether the data is valid or not; the provided implementation returns `true`.
    fn verify_data(&self, _state: S) -> bool {
        true
    }

    /// Processes the data; the provided implementation is a no-op.
    ///
    /// Kadmium doesn't make any assumptions about how this function should be implemented. E.g. if
    /// you want to run it asynchronously, you can pass in a handle to the runtime in the state and
    /// spawn a task; if you want to send it elsewhere for further processing, you could pass in a
    /// channel sender.
    fn process_data(&self, _state: S) {}
}
