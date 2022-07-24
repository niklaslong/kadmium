# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Byte array-backed `Id` to replace the `u128`. This also makes the code compilable on `stable` Rust.

### Changed

- Make the `buckets` field on `RoutingTable` private.
- The `find_k_closest` method now uses the log2 of the XOR-distance to order its search instead of the distance.

[unreleased]: https://github.com/niklaslong/kadmium/compare/v0.1.0...HEAD
