# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- APIs now use the connection address for lookups instead of the identifier.

### Removed

- The `set_last_seen` helper is no longer needed as part of the public or private APIs.

## [0.4.0]

### Added

- Introduce the `ProcessData<S>` trait to determine how to handle data wrapped in a `Chunk` message.

### Changed

- Make `process_message` and `process_chunk` generic over `ProcessData<S>`.
- Make `find_k_closest` private.

## [0.3.0]

### Added

- Show required features for gated APIs on docs.rs.
- Set `last_seen` when processing messages.

### Changed

- Pass in the sender's `Id` to `process_message`.
- Pass `Id` by reference where possible.

## [0.2.0]

### Added

- Byte array-backed `Id` to replace the `u128`.

### Removed

- Rust `nightly` feature `#![feature(int_log)]` requirement, crate compiles on `stable`.

### Changed

- Make the `buckets` field on `RoutingTable` private.
- The `find_k_closest` method now uses the log2 of the XOR-distance to order its search instead of the distance.
- Module visibility and re-exports to be more ergonomic.
- Various improvements to crate documentation.

[unreleased]: https://github.com/niklaslong/kadmium/compare/v0.1.0...HEAD
[0.4.0]: https://github.com/niklaslong/kadmium/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/niklaslong/kadmium/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/niklaslong/kadmium/compare/v0.1.0...v0.2.0
