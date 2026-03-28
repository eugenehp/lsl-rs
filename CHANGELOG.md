# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `rlsl-cli` — unified CLI tool combining gen, record, convert, bench, and list commands
- `rlsl-fuzz` — fuzz testing targets for protocol parsers and XML DOM
- `docs/ARCHITECTURE.md` — detailed architecture documentation with crate dependency graph
- `CONTRIBUTING.md` — contribution guidelines
- `SECURITY.md` — vulnerability disclosure policy
- `CHANGELOG.md` — this file
- `Dockerfile` — container image for rlsl-rec and rlsl-gen
- Comprehensive unit tests for `sample`, `xml_dom`, `postproc`, `signal_quality`, `clock`,
  `send_buffer`, `config`, and `stream_info` modules
- Criterion benchmarks for sample serialization, XML DOM operations, and push/pull throughput
- Workspace-level examples: `multi_stream.rs`, `markers.rs`, `recording.rs`
- `cargo-deny` configuration for license and dependency auditing
- Cross-platform CI matrix (Linux x86_64, macOS aarch64, Windows x86_64)
- npm publish workflow for WASM package

## [0.1.0] — 2026-03-28

### Added
- **rlsl**: Pure-Rust LSL implementation with full protocol 1.00 + 1.10 support
  - `StreamInfo` — stream metadata with XPath-like query matching
  - `StreamOutlet` — publish data on the network
  - `StreamInlet` — receive data with automatic recovery
  - UDP multicast/broadcast discovery (IPv4 + IPv6 dual-stack)
  - TCP data streaming with protocol handshake
  - NTP-like time correction probing
  - Timestamp post-processing (dejitter, clocksync, monotonize)
  - Mutable XML DOM for `<desc>` metadata
  - Signal quality metrics (SNR, jitter, dropout detection)
  - Config file loading (`lsl_api.cfg`)
- **rlsl-sys**: C ABI shared library — 162/162 `extern "C"` symbols, drop-in for liblsl
- **exg**: XDF file writer and `NumericSample` trait
- **rlsl-rec**: Recording engine with XDF, Parquet, and HDF5 output + ratatui TUI
- **rrlsl-rec-gui**: eGUI recorder with live signal viewer and stream inspector
- **rlsl-py**: Python bindings via PyO3 + numpy (StreamInfo, StreamOutlet, StreamInlet, resolver)
- **rlsl-wasm**: WebSocket bridge server + WASM browser client
- **rlsl-gen**: Synthetic signal generator (sine, square, noise, chirp, sawtooth, counter)
- **rlsl-bench**: Throughput and latency benchmarking tool
- **rlsl-convert**: Offline format converter (XDF → Parquet, XDF → CSV, Parquet → CSV)
- CI workflows for lint, test (Linux/macOS/Windows), WASM, and Python wheels
- Release workflow with GitHub Releases + PyPI publishing

### Known Gaps
- mDNS/Bonjour discovery not implemented (LSL uses UDP multicast as primary discovery)

[Unreleased]: https://github.com/eugenehp/rlsl/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/eugenehp/rlsl/releases/tag/v0.1.0
