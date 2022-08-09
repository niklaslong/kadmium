//! Core routing table implementation fine-tuned for TCP.

mod router;
#[cfg(feature = "sync")]
mod sync;

pub(crate) use router::ConnState;
pub use router::RoutingTable;
#[cfg(feature = "sync")]
pub use sync::SyncRoutingTable;
