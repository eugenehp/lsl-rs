//! Per-chunk compression for the iroh tunnel.
//!
//! # Framing
//!
//! Every compressed chunk on the wire is:
//!
//! ```text
//! [u16 LE compressed_len] [u16 LE uncompressed_len] [compressed payload]
//! ```
//!
//! 4 bytes of overhead per chunk. If `compressed_len == uncompressed_len`
//! the payload is stored verbatim (incompressible data fallback).
//!
//! # Available codecs
//!
//! | Mode          | Codec          | Speed          | Ratio  | Use case                        |
//! |---------------|----------------|----------------|--------|---------------------------------|
//! | `None`        | —              | —              | 1.0×   | LAN, localhost                  |
//! | `Lz4`         | LZ4 block      | ~3 GB/s enc    | 1.5–2× | Default when bandwidth-limited  |
//! | `Zstd1`       | Zstandard L1   | ~800 MB/s enc  | 2–3×   | Balanced speed / ratio          |
//! | `Zstd3`       | Zstandard L3   | ~400 MB/s enc  | 2.5–4× | Better ratio, still fast        |
//! | `Snappy`      | Snappy         | ~2 GB/s enc    | 1.5–2× | Similar to LZ4, Google standard |
//! | `DeltaLz4`    | XOR-delta+LZ4  | ~2 GB/s enc    | 3–8×   | Best for numeric physio signals |

/// Compression mode negotiated in the stream header (1 byte on wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    /// No compression (default). Raw protocol-1.10 bytes.
    None = 0,
    /// LZ4 block compression.
    Lz4 = 1,
    /// Zstandard level 1 (fast).
    Zstd1 = 2,
    /// Zstandard level 3 (balanced).
    Zstd3 = 3,
    /// Google Snappy.
    Snappy = 4,
    /// XOR-delta encoding followed by LZ4.
    /// Dramatically improves ratio on numeric physiological signals
    /// where consecutive samples differ only slightly.
    DeltaLz4 = 5,
}

impl Compression {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Lz4,
            2 => Self::Zstd1,
            3 => Self::Zstd3,
            4 => Self::Snappy,
            5 => Self::DeltaLz4,
            _ => Self::None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Lz4 => "lz4",
            Self::Zstd1 => "zstd1",
            Self::Zstd3 => "zstd3",
            Self::Snappy => "snappy",
            Self::DeltaLz4 => "delta-lz4",
        }
    }

    pub fn from_name(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "lz4" => Self::Lz4,
            "zstd" | "zstd1" => Self::Zstd1,
            "zstd3" | "zstd-high" => Self::Zstd3,
            "snappy" | "snap" => Self::Snappy,
            "delta-lz4" | "delta_lz4" | "dlz4" => Self::DeltaLz4,
            _ => Self::None,
        }
    }

    /// Returns true if this mode uses the chunked framing wire format.
    pub fn is_compressed(&self) -> bool {
        *self != Self::None
    }
}

impl std::fmt::Display for Compression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Compress ─────────────────────────────────────────────────────────

/// Compress a chunk of raw sample bytes using the given codec.
/// Output: `[u16 compressed_len][u16 uncompressed_len][payload]`.
///
/// Falls back to storing uncompressed if the codec expands the data.
pub fn compress_chunk(raw: &[u8], mode: Compression, out: &mut Vec<u8>) {
    debug_assert!(raw.len() <= u16::MAX as usize);
    let uncompressed_len = raw.len();

    let compressed_payload = match mode {
        Compression::None => unreachable!("compress_chunk called with None"),
        Compression::Lz4 => compress_lz4(raw),
        Compression::Zstd1 => compress_zstd(raw, 1),
        Compression::Zstd3 => compress_zstd(raw, 3),
        Compression::Snappy => compress_snappy(raw),
        Compression::DeltaLz4 => {
            let delta = delta_encode(raw);
            compress_lz4(&delta)
        }
    };

    if compressed_payload.len() < uncompressed_len {
        out.extend_from_slice(&(compressed_payload.len() as u16).to_le_bytes());
        out.extend_from_slice(&(uncompressed_len as u16).to_le_bytes());
        out.extend_from_slice(&compressed_payload);
    } else {
        // Store uncompressed — sentinel: compressed_len == uncompressed_len
        out.extend_from_slice(&(uncompressed_len as u16).to_le_bytes());
        out.extend_from_slice(&(uncompressed_len as u16).to_le_bytes());
        out.extend_from_slice(raw);
    }
}

/// Decompress a framed chunk.
/// Returns `(decompressed_bytes, total_bytes_consumed_from_input)`.
/// Returns `None` if the input is too short (need more data).
pub fn decompress_chunk(input: &[u8], mode: Compression) -> Option<(Vec<u8>, usize)> {
    if input.len() < 4 {
        return None;
    }
    let compressed_len = u16::from_le_bytes([input[0], input[1]]) as usize;
    let uncompressed_len = u16::from_le_bytes([input[2], input[3]]) as usize;
    let total = 4 + compressed_len;

    if input.len() < total {
        return None;
    }

    let payload = &input[4..total];

    if compressed_len == uncompressed_len {
        // Stored uncompressed
        return Some((payload.to_vec(), total));
    }

    let decompressed = match mode {
        Compression::None => unreachable!(),
        Compression::Lz4 => decompress_lz4(payload, uncompressed_len)?,
        Compression::Zstd1 | Compression::Zstd3 => decompress_zstd(payload)?,
        Compression::Snappy => decompress_snappy(payload)?,
        Compression::DeltaLz4 => {
            let delta = decompress_lz4(payload, uncompressed_len)?;
            delta_decode(&delta)
        }
    };

    Some((decompressed, total))
}

// ── Codec implementations ────────────────────────────────────────────

fn compress_lz4(data: &[u8]) -> Vec<u8> {
    let with_size = lz4_flex::compress_prepend_size(data);
    // Strip the 4-byte original-size prefix that lz4_flex adds
    with_size[4..].to_vec()
}

fn decompress_lz4(data: &[u8], uncompressed_len: usize) -> Option<Vec<u8>> {
    lz4_flex::decompress(data, uncompressed_len).ok()
}

fn compress_zstd(data: &[u8], level: i32) -> Vec<u8> {
    zstd::bulk::compress(data, level).unwrap_or_else(|_| data.to_vec())
}

fn decompress_zstd(data: &[u8]) -> Option<Vec<u8>> {
    // zstd frame carries its own uncompressed size
    zstd::bulk::decompress(data, 65536).ok()
}

fn compress_snappy(data: &[u8]) -> Vec<u8> {
    let mut enc = snap::raw::Encoder::new();
    enc.compress_vec(data).unwrap_or_else(|_| data.to_vec())
}

fn decompress_snappy(data: &[u8]) -> Option<Vec<u8>> {
    let mut dec = snap::raw::Decoder::new();
    dec.decompress_vec(data).ok()
}

// ── Delta encoding ───────────────────────────────────────────────────

/// XOR-delta encode: output[0] = input[0], output[i] = input[i] ^ input[i-1].
///
/// For numeric physiological signals where consecutive bytes are similar,
/// most delta bytes become zero → extremely compressible by LZ4/zstd.
fn delta_encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(data.len());
    out.push(data[0]);
    for i in 1..data.len() {
        out.push(data[i] ^ data[i - 1]);
    }
    out
}

/// Reverse XOR-delta encoding.
fn delta_decode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(data.len());
    out.push(data[0]);
    for i in 1..data.len() {
        out.push(data[i] ^ out[i - 1]);
    }
    out
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(mode: Compression, data: &[u8]) {
        let mut compressed = Vec::new();
        compress_chunk(data, mode, &mut compressed);

        let (decompressed, consumed) = decompress_chunk(&compressed, mode).unwrap();
        assert_eq!(
            consumed,
            compressed.len(),
            "mode={:?}: consumed mismatch",
            mode
        );
        assert_eq!(decompressed, data, "mode={:?}: data mismatch", mode);
    }

    #[test]
    fn roundtrip_all_codecs_compressible() {
        let raw = vec![42u8; 1024];
        for mode in [
            Compression::Lz4,
            Compression::Zstd1,
            Compression::Zstd3,
            Compression::Snappy,
            Compression::DeltaLz4,
        ] {
            roundtrip(mode, &raw);
        }
    }

    #[test]
    fn roundtrip_all_codecs_random() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut raw = vec![0u8; 256];
        for (i, b) in raw.iter_mut().enumerate() {
            let mut h = DefaultHasher::new();
            i.hash(&mut h);
            *b = h.finish() as u8;
        }
        for mode in [
            Compression::Lz4,
            Compression::Zstd1,
            Compression::Zstd3,
            Compression::Snappy,
            Compression::DeltaLz4,
        ] {
            roundtrip(mode, &raw);
        }
    }

    #[test]
    fn roundtrip_all_codecs_tiny() {
        // Minimal: 1 byte
        for mode in [
            Compression::Lz4,
            Compression::Zstd1,
            Compression::Zstd3,
            Compression::Snappy,
            Compression::DeltaLz4,
        ] {
            roundtrip(mode, &[0xAB]);
        }
    }

    #[test]
    fn partial_input_returns_none() {
        for mode in [Compression::Lz4, Compression::Zstd1, Compression::Snappy] {
            assert!(decompress_chunk(&[], mode).is_none());
            assert!(decompress_chunk(&[1, 0, 2, 0], mode).is_none());
        }
    }

    #[test]
    fn delta_encode_decode_identity() {
        let data: Vec<u8> = (0..=255).collect();
        let encoded = delta_encode(&data);
        let decoded = delta_decode(&encoded);
        assert_eq!(decoded, data);
    }

    #[test]
    fn delta_encode_decode_empty() {
        assert!(delta_encode(&[]).is_empty());
        assert!(delta_decode(&[]).is_empty());
    }

    #[test]
    fn delta_lz4_compresses_better_than_plain_lz4() {
        // Slowly changing signal — delta should compress much better
        let mut raw = vec![0u8; 1024];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = (100 + (i / 32) as u8).wrapping_add((i % 4) as u8);
        }

        let mut lz4_out = Vec::new();
        compress_chunk(&raw, Compression::Lz4, &mut lz4_out);

        let mut delta_out = Vec::new();
        compress_chunk(&raw, Compression::DeltaLz4, &mut delta_out);

        assert!(
            delta_out.len() <= lz4_out.len(),
            "delta-lz4 ({}) should be <= lz4 ({}) for correlated data",
            delta_out.len(),
            lz4_out.len()
        );
    }

    #[test]
    fn from_name_variants() {
        assert_eq!(Compression::from_name("none"), Compression::None);
        assert_eq!(Compression::from_name("lz4"), Compression::Lz4);
        assert_eq!(Compression::from_name("zstd"), Compression::Zstd1);
        assert_eq!(Compression::from_name("zstd1"), Compression::Zstd1);
        assert_eq!(Compression::from_name("zstd3"), Compression::Zstd3);
        assert_eq!(Compression::from_name("zstd-high"), Compression::Zstd3);
        assert_eq!(Compression::from_name("snappy"), Compression::Snappy);
        assert_eq!(Compression::from_name("snap"), Compression::Snappy);
        assert_eq!(Compression::from_name("delta-lz4"), Compression::DeltaLz4);
        assert_eq!(Compression::from_name("dlz4"), Compression::DeltaLz4);
        assert_eq!(Compression::from_name("garbage"), Compression::None);
    }
}
