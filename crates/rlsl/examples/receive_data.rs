//! Minimal inlet example — resolves a stream and pulls samples.
//!
//! Run with: cargo run --example receive_data

use rlsl::prelude::*;
use rlsl::resolver;

fn main() {
    println!("Resolving streams …");
    let results = resolver::resolve_all(2.0);
    if results.is_empty() {
        println!("No streams found.");
        return;
    }
    let info = &results[0];
    println!(
        "Found: name={} type={} ch={} srate={}",
        info.name(),
        info.type_(),
        info.channel_count(),
        info.nominal_srate()
    );

    let inlet = StreamInlet::new(info, 360, 0, true);
    inlet.open_stream(10.0).expect("open_stream failed");

    let nch = info.channel_count() as usize;
    let mut buf = vec![0.0f64; nch];
    loop {
        match inlet.pull_sample_d(&mut buf, 1.0) {
            Ok(ts) if ts > 0.0 => {
                print!("t={ts:.4}  ");
                for v in &buf {
                    print!("{v:.2} ");
                }
                println!();
            }
            _ => {}
        }
    }
}
