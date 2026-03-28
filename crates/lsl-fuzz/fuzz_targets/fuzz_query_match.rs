//! Fuzz StreamInfo query matching with arbitrary query strings.

#![no_main]
use libfuzzer_sys::fuzz_target;
use lsl_core::stream_info::StreamInfo;
use lsl_core::types::ChannelFormat;

fuzz_target!(|query: &str| {
    // Create a representative stream info
    let info = StreamInfo::new("TestStream", "EEG", 8, 250.0, ChannelFormat::Float32, "src1");
    // Should never panic regardless of query string
    let _ = info.matches_query(query);
});
