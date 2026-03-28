//! Fuzz StreamInfo XML parsing (from_shortinfo_message).

#![no_main]
use libfuzzer_sys::fuzz_target;
use lsl_core::stream_info::StreamInfo;

fuzz_target!(|xml: &str| {
    // Should never panic regardless of XML input
    let _ = StreamInfo::from_shortinfo_message(xml);
});
