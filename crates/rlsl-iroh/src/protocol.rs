//! Wire protocol for the iroh tunnel.
//!
//! We use a minimal framing on top of iroh QUIC streams:
//!
//! 1. **Stream header** (sent once when the uni stream opens):
//!    - 4 bytes magic `b"LSL\x01"`
//!    - 1 byte compression mode (0 = none, 1 = lz4)
//!    - 3 bytes reserved (zero)
//!    - The shortinfo XML as a length-prefixed blob (4-byte LE length + UTF-8)
//!
//! 2. **Sample frames** (repeated):
//!    - If compression == None: raw protocol-1.10 serialized samples.
//!    - If compression == Lz4: chunked frames, each:
//!      `[u16 compressed_len][u16 uncompressed_len][payload]`
//!      containing one or more protocol-1.10 samples.

use crate::compress::Compression;
use rlsl::stream_info::StreamInfo;

/// ALPN protocol identifier for LSL-over-iroh.
pub const LSL_ALPN: &[u8] = b"/rlsl/stream/1";

/// Magic bytes at the start of every stream header.
pub const MAGIC: &[u8; 4] = b"LSL\x01";

/// Encode stream header into bytes.
pub fn encode_stream_header(info: &StreamInfo, compression: Compression) -> Vec<u8> {
    let xml = info.to_shortinfo_message();
    let xml_bytes = xml.as_bytes();
    // magic(4) + compression(1) + reserved(3) + xml_len(4) + xml
    let mut buf = Vec::with_capacity(12 + xml_bytes.len());
    buf.extend_from_slice(MAGIC);
    buf.push(compression as u8);
    buf.extend_from_slice(&[0u8; 3]); // reserved
    buf.extend_from_slice(&(xml_bytes.len() as u32).to_le_bytes());
    buf.extend_from_slice(xml_bytes);
    buf
}

/// Decode stream header from raw bytes.
/// Returns (StreamInfo, Compression, bytes_consumed).
pub fn decode_stream_header(data: &[u8]) -> anyhow::Result<(StreamInfo, Compression, usize)> {
    anyhow::ensure!(data.len() >= 12, "header too short");
    anyhow::ensure!(&data[..4] == MAGIC, "bad magic");
    let compression = Compression::from_u8(data[4]);
    // data[5..8] reserved
    let xml_len = u32::from_le_bytes(data[8..12].try_into()?) as usize;
    anyhow::ensure!(data.len() >= 12 + xml_len, "header truncated");
    let xml = std::str::from_utf8(&data[12..12 + xml_len])?;
    let info =
        StreamInfo::from_shortinfo_message(xml).ok_or_else(|| anyhow::anyhow!("bad XML header"))?;
    Ok((info, compression, 12 + xml_len))
}
