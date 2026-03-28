//! Unit-level round-trip tests for the iroh tunnel wire protocol.

use rlsl::prelude::*;
use rlsl::sample::Sample;
use rlsl_iroh::compress::Compression;
use rlsl_iroh::protocol;

#[test]
fn stream_header_roundtrip_no_compression() {
    let info = StreamInfo::new("TestEEG", "EEG", 8, 256.0, ChannelFormat::Float32, "src42");
    let encoded = protocol::encode_stream_header(&info, Compression::None);

    let (decoded, comp, consumed) = protocol::decode_stream_header(&encoded).unwrap();
    assert_eq!(consumed, encoded.len());
    assert_eq!(comp, Compression::None);
    assert_eq!(decoded.name(), "TestEEG");
    assert_eq!(decoded.type_(), "EEG");
    assert_eq!(decoded.channel_count(), 8);
    assert!((decoded.nominal_srate() - 256.0).abs() < 1e-6);
    assert_eq!(decoded.channel_format(), ChannelFormat::Float32);
    assert_eq!(decoded.source_id(), "src42");
}

#[test]
fn stream_header_roundtrip_lz4() {
    let info = StreamInfo::new(
        "TestLZ4",
        "EMG",
        4,
        1000.0,
        ChannelFormat::Double64,
        "lz4src",
    );
    let encoded = protocol::encode_stream_header(&info, Compression::Lz4);

    let (decoded, comp, consumed) = protocol::decode_stream_header(&encoded).unwrap();
    assert_eq!(consumed, encoded.len());
    assert_eq!(comp, Compression::Lz4);
    assert_eq!(decoded.name(), "TestLZ4");
    assert_eq!(decoded.channel_format(), ChannelFormat::Double64);
}

#[test]
fn sample_110_roundtrip_float() {
    let fmt = ChannelFormat::Float32;
    let nch = 4u32;
    let mut sample = Sample::new(fmt, nch, 0.0);
    sample.timestamp = 42.5;
    sample.assign_f32(&[1.0, 2.0, 3.0, 4.0]);

    let mut buf = Vec::new();
    sample.serialize_110(&mut buf);

    let mut cursor = std::io::Cursor::new(&buf);
    let decoded = Sample::deserialize_110(&mut cursor, fmt, nch).unwrap();

    assert!((decoded.timestamp - 42.5).abs() < 1e-10);
    let mut out = [0.0f32; 4];
    decoded.retrieve_f32(&mut out);
    assert_eq!(out, [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn sample_110_roundtrip_string() {
    let fmt = ChannelFormat::String;
    let nch = 2u32;
    let mut sample = Sample::new(fmt, nch, 0.0);
    sample.timestamp = 99.0;
    sample.assign_strings(&["hello".to_string(), "world".to_string()]);

    let mut buf = Vec::new();
    sample.serialize_110(&mut buf);

    let mut cursor = std::io::Cursor::new(&buf);
    let decoded = Sample::deserialize_110(&mut cursor, fmt, nch).unwrap();

    assert!((decoded.timestamp - 99.0).abs() < 1e-10);
    assert_eq!(decoded.retrieve_strings(), vec!["hello", "world"]);
}

#[test]
fn multi_sample_parsing() {
    let fmt = ChannelFormat::Double64;
    let nch = 2u32;

    let mut buf = Vec::new();
    for i in 0..5 {
        let mut s = Sample::new(fmt, nch, 0.0);
        s.timestamp = 100.0 + i as f64;
        s.assign_f64(&[i as f64, (i * 10) as f64]);
        s.serialize_110(&mut buf);
    }

    let mut cursor = std::io::Cursor::new(&buf);
    let mut count = 0;
    loop {
        let pos = cursor.position() as usize;
        if pos >= buf.len() {
            break;
        }
        match Sample::deserialize_110(&mut cursor, fmt, nch) {
            Ok(s) => {
                assert!((s.timestamp - (100.0 + count as f64)).abs() < 1e-10);
                count += 1;
            }
            Err(_) => break,
        }
    }
    assert_eq!(count, 5);
}

#[test]
fn header_bad_magic_rejected() {
    let bad = b"BAD\x01\x00\x00\x00\x00\x00\x00\x00\x00";
    assert!(protocol::decode_stream_header(bad).is_err());
}

#[test]
fn header_truncated_rejected() {
    let info = StreamInfo::new("X", "Y", 1, 1.0, ChannelFormat::Int32, "");
    let encoded = protocol::encode_stream_header(&info, Compression::None);
    let truncated = &encoded[..10];
    assert!(protocol::decode_stream_header(truncated).is_err());
}

#[test]
fn all_codecs_compress_decompress_roundtrip() {
    use rlsl_iroh::compress;

    // Simulate a chunk of serialized samples (repeated data → compressible)
    let mut raw = Vec::new();
    for i in 0..10 {
        let mut s = Sample::new(ChannelFormat::Float32, 8, 0.0);
        s.timestamp = 1000.0 + i as f64 * 0.004;
        s.assign_f32(&[0.1; 8]);
        s.serialize_110(&mut raw);
    }

    for mode in [
        Compression::Lz4,
        Compression::Zstd1,
        Compression::Zstd3,
        Compression::Snappy,
        Compression::DeltaLz4,
    ] {
        let mut compressed = Vec::new();
        compress::compress_chunk(&raw, mode, &mut compressed);

        let (decompressed, consumed) = compress::decompress_chunk(&compressed, mode).unwrap();
        assert_eq!(consumed, compressed.len(), "mode={:?}", mode);
        assert_eq!(decompressed, raw, "mode={:?}", mode);
    }
}

#[test]
fn zero_loss_sample_serialize_deserialize() {
    // Verify every format round-trips without loss through 110 serialization
    for &fmt in &[
        ChannelFormat::Float32,
        ChannelFormat::Double64,
        ChannelFormat::Int32,
        ChannelFormat::Int16,
        ChannelFormat::Int8,
        ChannelFormat::Int64,
    ] {
        let nch = 4u32;
        let mut sample = Sample::new(fmt, nch, 0.0);
        sample.assign_test_pattern(7);

        let mut buf = Vec::new();
        sample.serialize_110(&mut buf);

        let mut cursor = std::io::Cursor::new(&buf);
        let decoded = Sample::deserialize_110(&mut cursor, fmt, nch).unwrap();
        assert_eq!(sample, decoded, "Round-trip failed for format {:?}", fmt);
        assert_eq!(
            cursor.position() as usize,
            buf.len(),
            "Not all bytes consumed for format {:?}",
            fmt
        );
    }
}
