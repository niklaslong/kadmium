//! A broadcast protocol implementation based on [Kadcast](https://eprint.iacr.org/2021/996.pdf).
//!
//! Notable differences with the paper:
//!
//! 1. The implementation is optimised for TCP (not UDP), thus we assume the connection with active
//!    peers to be sufficiently reliable over time to avoid duplicating broadcasts.
//! 2. The buckets store only the current active connections, as opposed to all the connections in
//!    the original protocol.
//! 3. TODO: threat model differences and mitigations.

#![cfg_attr(doc_cfg, feature(doc_cfg))]

#[cfg(feature = "codec")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "codec")))]
pub mod codec;
pub mod message;

mod id;
mod router;
mod traits;

pub use id::Id;
pub use router::RoutingTable;
pub use traits::ProcessData;
