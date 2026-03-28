//! `rlsl-fuzz` — fuzz testing targets for rlsl.
//!
//! Run with:
//! ```sh
//! cargo +nightly fuzz run fuzz_sample_110 -- -max_total_time=300
//! cargo +nightly fuzz run fuzz_sample_100 -- -max_total_time=300
//! cargo +nightly fuzz run fuzz_xml_dom -- -max_total_time=300
//! cargo +nightly fuzz run fuzz_query_match -- -max_total_time=300
//! cargo +nightly fuzz run fuzz_stream_info_xml -- -max_total_time=300
//! ```
