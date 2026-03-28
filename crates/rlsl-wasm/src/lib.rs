//! `rlsl-wasm` — shared types for the LSL WebSocket bridge protocol.
//!
//! The bridge server (`lsl-bridge`) discovers LSL streams and pushes
//! data over WebSocket.  The WASM client receives it in the browser.
//!
//! ## Protocol (JSON over WebSocket)
//!
//! **Server → Client:**
//!
//! ```json
//! {"type":"streams","streams":[{"name":"EEG","type_":"EEG","channel_count":8,...}]}
//! {"type":"data","stream_id":"<uid>","timestamps":[1.0,1.004,...],"data":[[ch0,ch1,...],[...]]}
//! ```
//!
//! **Client → Server:**
//!
//! ```json
//! {"type":"subscribe","stream_id":"<uid>"}
//! {"type":"unsubscribe","stream_id":"<uid>"}
//! {"type":"list"}
//! ```

pub mod protocol;

#[cfg(feature = "wasm")]
pub mod wasm_client;
