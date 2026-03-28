#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

BIN=./target/release/rlsl-iroh
OUT=./figures/bench_data.csv

echo "codec,channels,srate,duration,format,pushed,received,loss_pct,throughput,data_rate_mb,rtt_us,lat_min_us,lat_mean_us,lat_p50_us,lat_p95_us,lat_p99_us,lat_max_us" > "$OUT"

parse() {
    local codec="$1" channels="$2" srate="$3" duration="$4" format="$5"
    local output
    output=$(RUST_LOG=error "$BIN" bench \
        --channels "$channels" --srate "$srate" --duration "$duration" \
        --format "$format" --compress "$codec" 2>&1)

    local pushed received loss throughput data_rate rtt lat_min lat_mean lat_p50 lat_p95 lat_p99 lat_max
    pushed=$(echo "$output" | grep "Pushed:" | grep -oE '[0-9]+')
    received=$(echo "$output" | grep "Received:" | grep -oE '[0-9]+')
    loss=$(echo "$output" | grep "Loss:" | grep -oE '[0-9]+\.[0-9]+')
    throughput=$(echo "$output" | grep "Throughput:" | grep -oE '[0-9]+')
    data_rate=$(echo "$output" | grep "Data rate:" | grep -oE '[0-9]+\.[0-9]+')
    rtt=$(echo "$output" | grep "QUIC RTT:" | grep -oE '[0-9]+' | head -1)
    lat_min=$(echo "$output" | grep "min:" | grep -oE '[0-9]+\.[0-9]+')
    lat_mean=$(echo "$output" | grep "mean:" | grep -oE '[0-9]+\.[0-9]+')
    lat_p50=$(echo "$output" | grep "p50:" | grep -oE '[0-9]+\.[0-9]+')
    lat_p95=$(echo "$output" | grep "p95:" | grep -oE '[0-9]+\.[0-9]+')
    lat_p99=$(echo "$output" | grep "p99:" | grep -oE '[0-9]+\.[0-9]+')
    lat_max=$(echo "$output" | grep "max:" | grep -oE '[0-9]+\.[0-9]+')

    echo "$codec,$channels,$srate,$duration,$format,$pushed,$received,$loss,$throughput,$data_rate,$rtt,$lat_min,$lat_mean,$lat_p50,$lat_p95,$lat_p99,$lat_max" >> "$OUT"
    echo "  ✓ $codec ${channels}ch ${srate}Hz $format → mean=${lat_mean}µs loss=${loss}%"
}

echo "═══ Sweep 1: Codec comparison (64ch × 2kHz × 3s, float32) ═══"
for codec in none lz4 zstd zstd3 snappy delta-lz4; do
    parse "$codec" 64 2000 3 float32
done

echo ""
echo "═══ Sweep 2: Channel count scaling (none, 1kHz × 3s, float32) ═══"
for ch in 1 4 8 16 32 64 128 256; do
    parse none "$ch" 1000 3 float32
done

echo ""
echo "═══ Sweep 3: Sample rate scaling (none, 8ch × 3s, float32) ═══"
for sr in 100 250 500 1000 2000 5000 10000; do
    parse none 8 "$sr" 3 float32
done

echo ""
echo "═══ Sweep 4: Format comparison (none, 32ch × 1kHz × 3s) ═══"
for fmt in float32 double64 int32 int16 int8 int64; do
    parse none 32 1000 3 "$fmt"
done

echo ""
echo "═══ Sweep 5: High-throughput codec comparison (256ch × 10kHz × 3s) ═══"
for codec in none lz4 zstd zstd3 snappy delta-lz4; do
    parse "$codec" 256 10000 3 float32
done

echo ""
echo "═══ Sweep 6: delta-lz4 vs none at increasing channel counts (2kHz × 3s) ═══"
for ch in 8 32 64 128 256; do
    parse none "$ch" 2000 3 float32
    parse delta-lz4 "$ch" 2000 3 float32
done

echo ""
echo "Done. Data saved to $OUT"
