//! Shared protocol types for the LSL WebSocket bridge.

use serde::{Deserialize, Serialize};

// ── Server → Client messages ─────────────────────────────────────────

/// Envelope for server messages.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    /// List of available streams.
    Streams { streams: Vec<StreamDesc> },
    /// A chunk of sample data for a subscribed stream.
    Data {
        stream_id: String,
        timestamps: Vec<f64>,
        /// Row-major: data[sample_idx][channel_idx]
        data: Vec<Vec<f64>>,
    },
    /// Error message.
    Error { message: String },
}

/// Description of a discovered LSL stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamDesc {
    pub uid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub channel_count: u32,
    pub nominal_srate: f64,
    pub channel_format: String,
    pub hostname: String,
    pub source_id: String,
}

// ── Client → Server messages ─────────────────────────────────────────

/// Envelope for client commands.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMsg {
    /// Request the list of available streams.
    List,
    /// Subscribe to a stream by uid.
    Subscribe { stream_id: String },
    /// Unsubscribe from a stream.
    Unsubscribe { stream_id: String },
}
