//! A broadcast protocol implementation based on [Kadcast](https://eprint.iacr.org/2021/996.pdf).
//!
//! This crate features multiple router implementations that can broadly be categorised by the
//! transport protocol they are optimised for. It currently supports
//! [TCP](https://datatracker.ietf.org/doc/html/rfc793) and aims to support
//! [QUIC](https://datatracker.ietf.org/doc/html/rfc9000) in future.

#![cfg_attr(doc_cfg, feature(doc_cfg))]

#[cfg(feature = "codec")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "codec")))]
pub mod codec;
pub mod tcp;

mod core;

// Feature flag compilation for now, since this is WIP.
#[cfg(feature = "quic")]
mod quic;

pub use crate::core::{id::Id, message, traits::ProcessData};
