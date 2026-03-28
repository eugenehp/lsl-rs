//! `rlsl-iroh` — Iroh QUIC tunnel bridge for LSL streams.
//!
//! Bridges local LSL streams over iroh's peer-to-peer QUIC connections.
//! The existing LSL TCP/UDP infrastructure is untouched — legacy clients
//! always work as before. This crate adds a parallel transport path:
//!
//! * **Source** — subscribes to local LSL outlets via `StreamInlet`,
//!   serializes samples, and pushes them over an iroh uni-directional stream.
//! * **Sink** — receives samples over iroh and re-publishes them through
//!   a local `StreamOutlet` so any standard LSL client can consume them.
//!
//! # Latency design
//!
//! * One iroh uni stream per LSL stream — no head-of-line blocking between
//!   independent streams.
//! * Samples are serialized with the compact protocol-1.10 wire format
//!   (1-byte tag + 8-byte timestamp + raw channel data) — zero extra framing.
//! * `set_nodelay(true)` equivalent via QUIC — no Nagle.
//! * The source pushes every sample immediately (no batching) when the LSL
//!   outlet's `pushthrough` flag is set.
//! * Optional QUIC datagram mode for fire-and-forget ultra-low-latency
//!   (lossy) transport — useful for high-rate physiological signals where
//!   a dropped sample is acceptable.

pub mod bench;
pub mod compress;
pub mod protocol;
pub mod sink;
pub mod source;
pub mod ticket;
