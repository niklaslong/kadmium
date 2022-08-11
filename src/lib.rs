//! A broadcast protocol implementation based on [Kadcast](https://eprint.iacr.org/2021/996.pdf).
//!
//! This crate features multiple router implementations that can broadly be categorised by the
//! transport protocol they are optimised for. It currently supports
//! [TCP](https://datatracker.ietf.org/doc/html/rfc793) and
//! [QUIC](https://datatracker.ietf.org/doc/html/rfc9000).

#![cfg_attr(doc_cfg, feature(doc_cfg))]

#[cfg(feature = "codec")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "codec")))]
pub mod codec;
// TODO: feature gate.
pub mod quic;
pub mod tcp;

mod core;

pub use crate::core::{id::Id, message, traits::ProcessData};
