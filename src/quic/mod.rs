//! Core router implementations fine-tuned for QUIC.

mod router;
mod sync;

pub use router::QuicRouter;
pub use sync::SyncQuicRouter;
