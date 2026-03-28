//! `rlsl-iroh` — CLI for tunneling LSL streams over iroh.
//!
//! ## Usage
//!
//! ```sh
//! # Start the sink (receiver) — prints its Node ID:
//! rlsl-iroh sink
//!
//! # Start the source (sender) — connects to the sink:
//! rlsl-iroh source --sink <NODE_ID>
//!
//! # With LZ4 compression (saves bandwidth, ~5µs extra latency):
//! rlsl-iroh source --sink <NODE_ID> --compress lz4
//!
//! # List local LSL streams:
//! rlsl-iroh list
//!
//! # Benchmark the iroh tunnel (in-process, no extra setup):
//! rlsl-iroh bench
//! rlsl-iroh bench --channels 32 --srate 2000 --duration 10
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use iroh::endpoint::presets::N0;
use iroh::endpoint::Endpoint;
use iroh::protocol::Router;
use iroh::PublicKey;
use rlsl::prelude::ChannelFormat;
use rlsl_iroh::bench::{self, low_latency_transport, BenchConfig};
use rlsl_iroh::compress::Compression;
use rlsl_iroh::protocol::LSL_ALPN;
use rlsl_iroh::sink::{run_sink, LslSinkHandler};
use rlsl_iroh::source::{self, SourceConfig};

#[derive(Parser)]
#[command(name = "rlsl-iroh", about = "Tunnel LSL streams over iroh (QUIC P2P)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run as a sink (receiver): accept tunneled streams and re-publish locally.
    Sink,

    /// Run as a source (sender): resolve local LSL streams and forward to a sink.
    /// Uses reliable QUIC streams with zero data loss guarantee by default.
    Source {
        /// Node ID of the remote sink (hex public key).
        #[arg(long)]
        sink: String,

        /// LSL query to filter which streams to forward (e.g. "name='MyEEG'").
        #[arg(long, default_value = "")]
        query: String,

        /// Timeout in seconds for resolving local LSL streams.
        #[arg(long, default_value_t = 5.0)]
        resolve_timeout: f64,

        /// Continuously watch for new streams and forward them as they appear.
        #[arg(long, default_value_t = false)]
        continuous: bool,

        /// Compression: none, lz4, zstd, zstd3, snappy, delta-lz4.
        #[arg(long, default_value = "none")]
        compress: String,

        /// Use lossy QUIC datagrams instead of reliable streams.
        /// WARNING: WILL drop samples under congestion. Requires
        /// the `lossy-datagrams` cargo feature.
        #[cfg(feature = "lossy-datagrams")]
        #[arg(long, default_value_t = false)]
        datagrams: bool,
    },

    /// List locally discoverable LSL streams.
    List {
        /// Timeout in seconds.
        #[arg(long, default_value_t = 2.0)]
        timeout: f64,
    },

    /// Benchmark iroh tunnel latency and throughput (in-process, no setup needed).
    Bench {
        /// Number of channels per sample.
        #[arg(long, default_value_t = 8)]
        channels: u32,

        /// Sample rate in Hz.
        #[arg(long, default_value_t = 1000.0)]
        srate: f64,

        /// Duration in seconds.
        #[arg(long, default_value_t = 5.0)]
        duration: f64,

        /// Channel format: float32, double64, int32, int16, int8, int64.
        #[arg(long, default_value = "float32")]
        format: String,

        /// Compression: none, lz4, zstd, zstd3, snappy, delta-lz4.
        #[arg(long, default_value = "none")]
        compress: String,

        /// Also run an in-process baseline (no iroh) for comparison.
        #[arg(long, default_value_t = false)]
        baseline: bool,
    },
}

fn parse_compression(s: &str) -> Compression {
    Compression::from_name(s)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();

    match cli.command {
        Command::Sink => {
            let endpoint: Endpoint = Endpoint::builder(N0)
                .transport_config(low_latency_transport())
                .alpns(vec![LSL_ALPN.to_vec()])
                .bind()
                .await?;

            let _router = Router::builder(endpoint.clone())
                .accept(LSL_ALPN, LslSinkHandler)
                .spawn();

            run_sink(&endpoint).await?;
            endpoint.close().await;
        }

        Command::Source {
            sink,
            query,
            resolve_timeout,
            continuous,
            compress,
            #[cfg(feature = "lossy-datagrams")]
            datagrams,
        } => {
            let sink_node_id: PublicKey = sink.parse()?;
            let endpoint: Endpoint = Endpoint::builder(N0)
                .transport_config(low_latency_transport())
                .bind()
                .await?;

            let config = SourceConfig {
                query,
                sink_node_id,
                resolve_timeout,
                continuous,
                compression: parse_compression(&compress),
                #[cfg(feature = "lossy-datagrams")]
                use_datagrams: datagrams,
            };

            source::run_source(&endpoint, config).await?;
            endpoint.close().await;
        }

        Command::List { timeout } => {
            let streams =
                tokio::task::spawn_blocking(move || source::list_local_streams(timeout)).await?;

            if streams.is_empty() {
                println!("No LSL streams found.");
            } else {
                println!("Found {} stream(s):", streams.len());
                for s in &streams {
                    println!(
                        "  • {} [{}] — {} ch @ {} Hz (uid={})",
                        s.name(),
                        s.type_(),
                        s.channel_count(),
                        s.nominal_srate(),
                        s.uid()
                    );
                }
            }
        }

        Command::Bench {
            channels,
            srate,
            duration,
            format,
            compress,
            baseline,
        } => {
            let fmt = ChannelFormat::from_name(&format);
            let comp = parse_compression(&compress);

            if baseline {
                eprintln!("━━━ Baseline: in-process LSL (TCP loopback, no iroh) ━━━");
                run_lsl_baseline(channels, srate, duration, fmt).await?;
                eprintln!();
            }

            eprintln!("━━━ Iroh tunnel benchmark (compression={:?}) ━━━", comp);
            let config = BenchConfig {
                channels,
                sample_rate: srate,
                duration_secs: duration,
                format: fmt,
                use_datagrams: false,
                compression: comp,
            };

            let results = bench::run_bench(config).await?;
            eprintln!();
            eprintln!("{}", results);

            if results.loss_pct > 0.0 {
                eprintln!(
                    "⚠️  DATA LOSS DETECTED: {:.4}% — this is a bug!",
                    results.loss_pct
                );
                std::process::exit(1);
            } else {
                eprintln!("✅ Zero data loss confirmed.");
            }
        }
    }

    Ok(())
}

/// Run a baseline benchmark using only in-process LSL (TCP loopback).
async fn run_lsl_baseline(
    nch: u32,
    srate: f64,
    duration_secs: f64,
    fmt: ChannelFormat,
) -> Result<()> {
    tokio::task::spawn_blocking(move || run_lsl_baseline_sync(nch, srate, duration_secs, fmt))
        .await??;
    Ok(())
}

fn run_lsl_baseline_sync(
    nch: u32,
    srate: f64,
    duration_secs: f64,
    fmt: ChannelFormat,
) -> Result<()> {
    use rlsl::clock::local_clock;
    use rlsl::inlet::StreamInlet;
    use rlsl::outlet::StreamOutlet;
    use rlsl::resolver;
    use std::time::{Duration, Instant};

    let n_samples = (srate * duration_secs) as u64;

    eprintln!(
        "⚡ Baseline: {}ch × {}Hz × {:.1}s = {} samples ({})",
        nch,
        srate,
        duration_secs,
        n_samples,
        fmt.as_str()
    );

    let info = rlsl::stream_info::StreamInfo::new(
        "BaselineBench",
        "Benchmark",
        nch,
        srate,
        fmt,
        "baseline",
    );

    let outlet = StreamOutlet::new(&info, 0, 360);
    std::thread::sleep(Duration::from_millis(200));

    let streams = resolver::resolve_all(2.0);
    let s = streams
        .iter()
        .find(|s| s.name() == "BaselineBench")
        .expect("Could not find BaselineBench");
    let inlet = StreamInlet::new(s, 360, 0, false);
    inlet.open_stream(5.0).map_err(|e| anyhow::anyhow!(e))?;

    // Warmup
    let warm_data = vec![0.0f32; nch as usize];
    let mut warm_buf = vec![0.0f32; nch as usize];
    for _ in 0..100 {
        outlet.push_sample_f(&warm_data, 0.0, true);
        let _ = inlet.pull_sample_f(&mut warm_buf, 1.0);
    }

    let sender = {
        let data = vec![0.0f32; nch as usize];
        std::thread::spawn(move || {
            let interval = Duration::from_secs_f64(1.0 / srate);
            for _ in 0..n_samples {
                let ts = local_clock();
                outlet.push_sample_f(&data, ts, true);
                std::thread::sleep(interval);
            }
        })
    };

    let mut buf = vec![0.0f32; nch as usize];
    let mut latencies = Vec::with_capacity(n_samples as usize);
    let start = Instant::now();
    let deadline = start + Duration::from_secs_f64(duration_secs + 2.0);

    while Instant::now() < deadline && (latencies.len() as u64) < n_samples {
        match inlet.pull_sample_f(&mut buf, 0.5) {
            Ok(ts) if ts > 0.0 => latencies.push(local_clock() - ts),
            _ => {}
        }
    }
    let elapsed = start.elapsed();
    sender.join().unwrap();

    let received = latencies.len() as u64;
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let to_us = 1_000_000.0;
    let mean = if latencies.is_empty() {
        0.0
    } else {
        latencies.iter().sum::<f64>() / latencies.len() as f64
    };
    let pct = |p: usize| -> f64 {
        if latencies.is_empty() {
            return 0.0;
        }
        latencies[(latencies.len() * p / 100).min(latencies.len() - 1)]
    };
    let throughput = received as f64 / elapsed.as_secs_f64();
    let data_rate = throughput * nch as f64 * fmt.channel_bytes().max(1) as f64 / 1_000_000.0;
    let loss = if n_samples > 0 {
        (1.0 - received as f64 / n_samples as f64) * 100.0
    } else {
        0.0
    };

    eprintln!("╔═══════════════════════════════════════════╗");
    eprintln!("║     Baseline (LSL TCP loopback) Results   ║");
    eprintln!("╠═══════════════════════════════════════════╣");
    eprintln!("║ Pushed:     {:>10} samples            ║", n_samples);
    eprintln!("║ Received:   {:>10} samples            ║", received);
    eprintln!("║ Loss:       {:>9.2}%                  ║", loss);
    eprintln!(
        "║ Duration:   {:>9.2}s                  ║",
        elapsed.as_secs_f64()
    );
    eprintln!("║ Throughput: {:>9.0} samples/s           ║", throughput);
    eprintln!("║ Data rate:  {:>9.2} MB/s               ║", data_rate);
    eprintln!("╠═══════════════════════════════════════════╣");
    eprintln!("║ Latency (push→pull):                     ║");
    eprintln!(
        "║   min:  {:>9.1} µs                     ║",
        latencies.first().copied().unwrap_or(0.0) * to_us
    );
    eprintln!("║   mean: {:>9.1} µs                     ║", mean * to_us);
    eprintln!(
        "║   p50:  {:>9.1} µs                     ║",
        pct(50) * to_us
    );
    eprintln!(
        "║   p95:  {:>9.1} µs                     ║",
        pct(95) * to_us
    );
    eprintln!(
        "║   p99:  {:>9.1} µs                     ║",
        pct(99) * to_us
    );
    eprintln!(
        "║   max:  {:>9.1} µs                     ║",
        latencies.last().copied().unwrap_or(0.0) * to_us
    );
    eprintln!("╚═══════════════════════════════════════════╝");

    Ok(())
}
