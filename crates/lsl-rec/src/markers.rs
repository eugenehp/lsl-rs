//! Stream markers / annotations — inject timestamped event markers during recording.
//!
//! Creates a dedicated LSL string stream for markers so they are recorded
//! alongside data streams in XDF/Parquet.

use lsl_core::outlet::StreamOutlet;
use lsl_core::stream_info::StreamInfo;
use lsl_core::types::ChannelFormat;

/// A marker outlet that pushes string events with timestamps.
pub struct MarkerOutlet {
    outlet: StreamOutlet,
    count: u64,
}

impl MarkerOutlet {
    /// Create a new marker stream.
    pub fn new(name: &str) -> Self {
        let info = StreamInfo::new(
            name,
            "Markers",
            1,
            0.0, // irregular rate
            ChannelFormat::String,
            &format!("marker_{}", uuid::Uuid::new_v4()),
        );
        let outlet = StreamOutlet::new(&info, 0, 360);
        MarkerOutlet { outlet, count: 0 }
    }

    /// Push a marker event with the current timestamp.
    pub fn push(&mut self, label: &str) {
        self.count += 1;
        self.outlet.push_sample_str(&[label.to_string()], 0.0, true);
    }

    /// Push a marker with a custom timestamp.
    pub fn push_at(&mut self, label: &str, timestamp: f64) {
        self.count += 1;
        self.outlet.push_sample_str(&[label.to_string()], timestamp, true);
    }

    /// Number of markers pushed.
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get the stream info (useful for recording).
    pub fn info(&self) -> &StreamInfo {
        self.outlet.info()
    }
}
