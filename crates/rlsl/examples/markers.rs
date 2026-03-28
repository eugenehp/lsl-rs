//! Example: Send and receive marker (event) streams.
//!
//! Demonstrates irregular-rate string streams used for experimental events.
//!
//! Run with: cargo run --example markers

use rlsl::prelude::*;
use rlsl::resolver;
use std::time::Duration;

fn main() -> anyhow::Result<()> {
    // ── Create marker outlet ──
    let info = StreamInfo::new(
        "ExperimentMarkers",
        "Markers",
        1,
        IRREGULAR_RATE,
        ChannelFormat::String,
        "marker_example",
    );

    let desc = info.desc();
    desc.append_child_value("experiment", "visual_oddball");
    desc.append_child_value("protocol_version", "1.0");
    let channels = desc.append_child("channels");
    let ch = channels.append_child("channel");
    ch.append_child_value("label", "EventType");
    ch.append_child_value("type", "Marker");

    let outlet = StreamOutlet::new(&info, 0, 0);
    eprintln!("✓ Marker outlet created, port {}", info.v4data_port());

    // ── Resolve and connect inlet ──
    std::thread::sleep(Duration::from_millis(500));
    eprintln!("  Resolving...");
    let streams = resolver::resolve_by_property("name", "ExperimentMarkers", 1, 5.0);
    if streams.is_empty() {
        anyhow::bail!("Could not find ExperimentMarkers stream");
    }

    let inlet = StreamInlet::new(&streams[0], 32, 0, true);
    inlet.open_stream(5.0).map_err(|e| anyhow::anyhow!(e))?;
    eprintln!("✓ Inlet connected");
    eprintln!();

    // ── Simulate experiment events ──
    let events = [
        (0.5, "trial_start"),
        (0.2, "stimulus_standard"),
        (1.0, "response_correct"),
        (0.5, "trial_end"),
        (0.5, "trial_start"),
        (0.2, "stimulus_oddball"),
        (0.8, "response_correct"),
        (0.5, "trial_end"),
        (0.5, "trial_start"),
        (0.2, "stimulus_standard"),
        (1.5, "response_miss"),
        (0.5, "trial_end"),
        (0.0, "experiment_end"),
    ];

    for (delay, event) in &events {
        std::thread::sleep(Duration::from_secs_f64(*delay));

        // Send marker
        outlet.push_sample_str(&[event.to_string()], 0.0, true);
        eprintln!("  → sent: {}", event);

        // Receive (with small timeout since we're in same process)
        match inlet.pull_sample_str(1.0) {
            Ok((strings, ts)) if ts > 0.0 => {
                eprintln!("  ← recv: {} (ts={:.4})", strings[0], ts);
            }
            _ => {
                eprintln!("  ← (timeout)");
            }
        }
    }

    eprintln!();
    eprintln!("Done!");
    Ok(())
}
