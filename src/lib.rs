//! A broadcast protocol implementation based on [Kadcast](https://eprint.iacr.org/2021/996.pdf).
//!
//! This crate features multiple routing table implementations but can broadly be categorised by
//! the transport protocol they are optimised for. This library supports TCP and QUIC. Both
//! implementations also contain a `Sync` router suitable for use in async contexts.
//!
//! ## TCP
//!
//! Notable differences with the paper:
//!
//! 1. The implementation is optimised for TCP (not UDP), thus we assume the connection with active
//!    peers to be sufficiently reliable over time to avoid duplicating broadcasts.
//! 2. The buckets store only the current active connections, as opposed to all the connections in
//!    the original protocol.
//! 3. TODO: threat model differences and mitigations.
//!
//! ## QUIC
//!
//! Implements a QUIC-based Kadcast as described in the original paper.

#![cfg_attr(doc_cfg, feature(doc_cfg))]

#[cfg(feature = "codec")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "codec")))]
pub mod codec;
pub mod message;
// TODO: feature gate.
pub mod quic;
pub mod tcp;

mod id;
mod traits;

pub use id::Id;
pub use traits::ProcessData;
