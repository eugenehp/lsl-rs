//! Fuzz protocol 1.00 sample deserialization.

#![no_main]
use libfuzzer_sys::fuzz_target;
use rlsl::sample::Sample;
use rlsl::types::ChannelFormat;
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    if data.len() < 2 {
        return;
    }

    let fmt = match data[0] % 7 {
        0 => ChannelFormat::Float32,
        1 => ChannelFormat::Double64,
        2 => ChannelFormat::Int32,
        3 => ChannelFormat::Int16,
        4 => ChannelFormat::Int8,
        5 => ChannelFormat::Int64,
        _ => ChannelFormat::String,
    };

    let num_channels = (data[1] % 32) as u32 + 1;
    let payload = &data[2..];

    let mut cursor = Cursor::new(payload);
    let _ = Sample::deserialize_100(&mut cursor, fmt, num_channels);
});
