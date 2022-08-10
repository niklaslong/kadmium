//! Core router implementations fine-tuned for TCP.

mod router;
#[cfg(feature = "sync")]
mod sync;
#[cfg(feature = "sync")]
mod traits;

pub(crate) use router::ConnState;
pub use router::TcpRouter;
#[cfg(feature = "sync")]
pub use sync::SyncTcpRouter;
#[cfg(feature = "sync")]
pub use traits::Kadcast;
