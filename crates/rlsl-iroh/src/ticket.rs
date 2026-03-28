//! Connection ticket for easy exchange between source and sink.

use iroh::{EndpointAddr, PublicKey};
use serde::{Deserialize, Serialize};

/// A minimal connection ticket that can be shared out-of-band
/// (copy-paste, QR code, etc.) to connect source ↔ sink.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    /// The node's public key (used as the node ID / endpoint ID).
    pub node_id: String,
    /// Optional relay URL the node is reachable through.
    pub relay_url: Option<String>,
}

impl Ticket {
    pub fn new(addr: &EndpointAddr, node_id: PublicKey) -> Self {
        let relay_url = addr.relay_urls().next().map(|u| u.to_string());
        Ticket {
            node_id: node_id.to_string(),
            relay_url,
        }
    }

    /// Encode to a compact base64-JSON string for copy-paste.
    pub fn to_string_compact(&self) -> String {
        use base64::Engine;
        let json = serde_json::to_vec(self).expect("ticket serialize");
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&json)
    }

    /// Decode from the compact string.
    pub fn from_string_compact(s: &str) -> anyhow::Result<Self> {
        use base64::Engine;
        let json = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(s.trim())?;
        Ok(serde_json::from_slice(&json)?)
    }
}
