//! Fuzz protocol 1.10 sample deserialization.
//! Ensures that arbitrary bytes never cause panics, only clean errors.

#![no_main]
use libfuzzer_sys::fuzz_target;
use rlsl::sample::Sample;
use rlsl::types::ChannelFormat;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    // Use first byte to select format and second for channel count
    let fmt = match data[0] % 7 {
        0 => ChannelFormat::Float32,
        1 => ChannelFormat::Double64,
        2 => ChannelFormat::Int32,
        3 => ChannelFormat::Int16,
        4 => ChannelFormat::Int8,
        5 => ChannelFormat::Int64,
        _ => ChannelFormat::String,
    };

    let num_channels = (data[1] % 32) as u32 + 1; // 1..32 channels
    let payload = &data[2..];

    let mut cursor = Cursor::new(payload);
    // Should never panic, may return Err
    let _ = Sample::deserialize_110(&mut cursor, fmt, num_channels);
});
