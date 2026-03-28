//! Minimal outlet example — pushes float samples at 250 Hz.
//!
//! Run with: cargo run --example send_data

use rlsl::prelude::*;
use std::time::Duration;

fn main() {
    let info = StreamInfo::new(
        "RustSender",
        "EEG",
        8,
        250.0,
        ChannelFormat::Float32,
        "rust1",
    );
    let outlet = StreamOutlet::new(&info, 0, 360);
    println!(
        "Streaming on TCP port {} …  (Ctrl-C to stop)",
        info.v4data_port()
    );

    let mut sample = [0.0f32; 8];
    let mut counter = 0u64;
    loop {
        for ch in sample.iter_mut() {
            *ch = counter as f32;
        }
        outlet.push_sample_f(&sample, 0.0, true);
        counter += 1;
        std::thread::sleep(Duration::from_secs_f64(1.0 / 250.0));
    }
}
