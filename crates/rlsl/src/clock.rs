//! High-resolution local clock, matching liblsl's lsl_local_clock().
//!
//! Uses std::time::Instant as a monotonic clock source with nanosecond precision.

use once_cell::sync::Lazy;
use std::time::Instant;

static EPOCH: Lazy<Instant> = Lazy::new(Instant::now);

/// Return the current local clock time in seconds (monotonic, high-resolution).
/// Equivalent to liblsl's lsl_local_clock().
pub fn local_clock() -> f64 {
    let elapsed = EPOCH.elapsed();
    elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0
}
