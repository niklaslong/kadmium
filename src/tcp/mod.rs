//! Core router implementations fine-tuned for TCP.
//!
//! Notable differences with the paper:
//!
//! 1. The implementation is optimised for TCP (not UDP), thus we assume the connection with active
//!    peers to be sufficiently reliable over time to avoid duplicating broadcasts.
//! 2. The buckets store only the current active connections, as opposed to all the connections as
//!    described in the original protocol.
//! 3. TODO: threat model differences and mitigations.

mod router;
#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "sync")]
mod traits;

#[cfg(feature = "sync")]
pub(crate) use router::ConnState;
pub use router::TcpRouter;
#[cfg(feature = "sync")]
pub use sync::SyncTcpRouter;
#[cfg(feature = "sync")]
pub use traits::Kadcast;
