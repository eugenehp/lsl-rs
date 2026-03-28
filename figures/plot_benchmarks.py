#!/usr/bin/env python3
"""Generate benchmark charts from bench_data.csv."""

import csv
import os
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

OUT = os.path.dirname(os.path.abspath(__file__))
CSV = os.path.join(OUT, "bench_data.csv")

# ── Load data ─────────────────────────────────────────────────────────

rows = []
with open(CSV) as f:
    reader = csv.DictReader(f)
    for r in reader:
        for k in r:
            try:
                r[k] = float(r[k])
            except (ValueError, TypeError):
                pass
        rows.append(r)

def select(rows, **kw):
    out = []
    for r in rows:
        match = True
        for k, v in kw.items():
            if isinstance(v, list):
                if r[k] not in v:
                    match = False
            elif r[k] != v:
                match = False
        if match:
            out.append(r)
    return out

COLORS = {
    "none": "#2196F3",
    "lz4": "#4CAF50",
    "zstd": "#FF9800",
    "zstd3": "#F44336",
    "snappy": "#9C27B0",
    "delta-lz4": "#00BCD4",
}

plt.rcParams.update({
    "figure.facecolor": "white",
    "axes.facecolor": "#FAFAFA",
    "axes.grid": True,
    "grid.alpha": 0.3,
    "font.size": 11,
    "axes.titlesize": 13,
    "axes.titleweight": "bold",
})

# ── Chart 1: Codec Latency Comparison (64ch × 2kHz) ──────────────────

fig, ax = plt.subplots(figsize=(10, 5))
order = ["none", "lz4", "snappy", "delta-lz4", "zstd", "zstd3"]
data_raw = select(rows, channels=64.0, srate=2000.0, format="float32",
              codec=list(COLORS.keys()))
# De-duplicate: keep first occurrence per codec
seen = set()
data = []
for r in data_raw:
    if r["codec"] not in seen:
        seen.add(r["codec"])
        data.append(r)
data.sort(key=lambda r: order.index(r["codec"]) if r["codec"] in order else 99)

codecs = [r["codec"] for r in data]
x = np.arange(len(codecs))
width = 0.5

means = [r["lat_mean_us"] for r in data]
p99s = [r["lat_p99_us"] for r in data]
maxs = [r["lat_max_us"] for r in data]
colors = [COLORS.get(c, "#888") for c in codecs]

bars = ax.bar(x, means, width, color=colors, alpha=0.85, label="Mean")
# Error bars showing p99 - mean
ax.errorbar(x, means, yerr=[np.zeros(len(means)), [p - m for p, m in zip(p99s, means)]],
            fmt="none", ecolor="black", capsize=4, capthick=1.5, linewidth=1.5)

for i, (m, p) in enumerate(zip(means, p99s)):
    ax.text(i, m + 0.5, f"{m:.1f}", ha="center", va="bottom", fontsize=9, fontweight="bold")
    ax.text(i, p + 0.5, f"p99={p:.0f}", ha="center", va="bottom", fontsize=8, color="#555")

ax.set_xticks(x)
ax.set_xticklabels(codecs, fontsize=11)
ax.set_ylabel("Latency (µs)")
ax.set_title("Codec Latency Comparison — 64ch × 2kHz float32")
ax.set_ylim(0, max(p99s) * 1.3)
ax.axhline(y=0, color="black", linewidth=0.5)
fig.tight_layout()
fig.savefig(os.path.join(OUT, "01_codec_latency.png"), dpi=150)
plt.close(fig)
print("  ✓ 01_codec_latency.png")

# ── Chart 2: Latency Percentiles (box-style) ─────────────────────────

fig, ax = plt.subplots(figsize=(10, 5))
for i, r in enumerate(data):
    c = COLORS.get(r["codec"], "#888")
    positions = [r["lat_min_us"], r["lat_p50_us"], r["lat_mean_us"], r["lat_p95_us"], r["lat_p99_us"], r["lat_max_us"]]
    labels_row = ["min", "p50", "mean", "p95", "p99", "max"]
    ax.plot(positions, [i]*len(positions), "o-", color=c, markersize=6, linewidth=2, label=r["codec"])
    ax.plot(r["lat_mean_us"], i, "D", color=c, markersize=9, zorder=5)

ax.set_yticks(range(len(data)))
ax.set_yticklabels([r["codec"] for r in data])
ax.set_xlabel("Latency (µs)")
ax.set_title("Latency Distribution by Codec — 64ch × 2kHz float32")
ax.legend(loc="lower right", fontsize=9)
# Add percentile labels at top
for lbl, xpos in [("min", data[0]["lat_min_us"]), ("max", data[0]["lat_max_us"])]:
    ax.annotate(lbl, (xpos, len(data)-0.5), fontsize=8, ha="center", color="#777")
fig.tight_layout()
fig.savefig(os.path.join(OUT, "02_latency_percentiles.png"), dpi=150)
plt.close(fig)
print("  ✓ 02_latency_percentiles.png")

# ── Chart 3: Channel Count Scaling ───────────────────────────────────

fig, ax = plt.subplots(figsize=(9, 5))
data = select(rows, codec="none", srate=1000.0, format="float32")
data.sort(key=lambda r: r["channels"])
chs = [int(r["channels"]) for r in data]
means = [r["lat_mean_us"] for r in data]
p99s = [r["lat_p99_us"] for r in data]

ax.plot(chs, means, "o-", color=COLORS["none"], linewidth=2, markersize=7, label="Mean")
ax.plot(chs, p99s, "s--", color="#F44336", linewidth=1.5, markersize=6, label="p99")
ax.fill_between(chs, means, p99s, alpha=0.1, color="#F44336")

for ch, m in zip(chs, means):
    ax.text(ch, m - 2.5, f"{m:.1f}", ha="center", fontsize=8, color=COLORS["none"])

ax.set_xlabel("Channels per sample")
ax.set_ylabel("Latency (µs)")
ax.set_title("Latency vs Channel Count — no compression, 1kHz float32")
ax.set_xscale("log", base=2)
ax.set_xticks(chs)
ax.set_xticklabels([str(c) for c in chs])
ax.legend()
fig.tight_layout()
fig.savefig(os.path.join(OUT, "03_channel_scaling.png"), dpi=150)
plt.close(fig)
print("  ✓ 03_channel_scaling.png")

# ── Chart 4: Sample Rate Scaling ─────────────────────────────────────

fig, ax = plt.subplots(figsize=(9, 5))
data = select(rows, codec="none", channels=8.0, format="float32")
data.sort(key=lambda r: r["srate"])
srs = [int(r["srate"]) for r in data]
means = [r["lat_mean_us"] for r in data]
p99s = [r["lat_p99_us"] for r in data]
thrus = [r["data_rate_mb"] for r in data]

ax.plot(srs, means, "o-", color=COLORS["none"], linewidth=2, markersize=7, label="Mean latency")
ax.plot(srs, p99s, "s--", color="#F44336", linewidth=1.5, markersize=6, label="p99 latency")
ax.fill_between(srs, means, p99s, alpha=0.1, color="#F44336")

ax.set_xlabel("Sample rate (Hz)")
ax.set_ylabel("Latency (µs)")
ax.set_title("Latency vs Sample Rate — no compression, 8ch float32")
ax.set_xscale("log")
ax.legend(loc="upper right")

ax2 = ax.twinx()
ax2.plot(srs, thrus, "^-", color="#9C27B0", linewidth=1.5, markersize=6, alpha=0.7)
ax2.set_ylabel("Data rate (MB/s)", color="#9C27B0")
ax2.tick_params(axis="y", labelcolor="#9C27B0")

fig.tight_layout()
fig.savefig(os.path.join(OUT, "04_srate_scaling.png"), dpi=150)
plt.close(fig)
print("  ✓ 04_srate_scaling.png")

# ── Chart 5: Format Comparison ────────────────────────────────────────

fig, ax = plt.subplots(figsize=(9, 5))
data = select(rows, codec="none", channels=32.0, srate=1000.0)
fmt_order = ["int8", "int16", "int32", "float32", "int64", "double64"]
data.sort(key=lambda r: fmt_order.index(r["format"]) if r["format"] in fmt_order else 99)

fmts = [r["format"] for r in data]
means = [r["lat_mean_us"] for r in data]
p99s = [r["lat_p99_us"] for r in data]
# Bytes per sample
bps = {"int8": 32, "int16": 64, "int32": 128, "float32": 128, "int64": 256, "double64": 256}

x = np.arange(len(fmts))
bars = ax.bar(x, means, 0.5, color=[COLORS["none"]]*len(fmts), alpha=0.85)
ax.errorbar(x, means, yerr=[np.zeros(len(means)), [p-m for p,m in zip(p99s, means)]],
            fmt="none", ecolor="black", capsize=4, capthick=1.5)

for i, (f, m) in enumerate(zip(fmts, means)):
    ax.text(i, m + 0.5, f"{m:.1f}µs", ha="center", va="bottom", fontsize=9, fontweight="bold")
    ax.text(i, 2, f"{bps.get(f,0)} B/samp", ha="center", fontsize=8, color="white", fontweight="bold")

ax.set_xticks(x)
ax.set_xticklabels(fmts)
ax.set_ylabel("Latency (µs)")
ax.set_title("Latency by Channel Format — 32ch × 1kHz, no compression")
ax.set_ylim(0, max(p99s) * 1.25)
fig.tight_layout()
fig.savefig(os.path.join(OUT, "05_format_comparison.png"), dpi=150)
plt.close(fig)
print("  ✓ 05_format_comparison.png")

# ── Chart 6: High-throughput Codec Comparison (256ch × 10kHz) ────────

fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(13, 5))
data = select(rows, channels=256.0, srate=10000.0, format="float32")
data.sort(key=lambda r: order.index(r["codec"]) if r["codec"] in order else 99)

codecs = [r["codec"] for r in data]
means = [r["lat_mean_us"] for r in data]
p99s = [r["lat_p99_us"] for r in data]
thrus = [r["data_rate_mb"] for r in data]
colors = [COLORS.get(c, "#888") for c in codecs]

x = np.arange(len(codecs))
ax1.bar(x, means, 0.5, color=colors, alpha=0.85)
ax1.errorbar(x, means, yerr=[np.zeros(len(means)), [p-m for p,m in zip(p99s, means)]],
             fmt="none", ecolor="black", capsize=4, capthick=1.5)
for i, m in enumerate(means):
    ax1.text(i, m + 0.5, f"{m:.1f}", ha="center", fontsize=9, fontweight="bold")
ax1.set_xticks(x)
ax1.set_xticklabels(codecs, rotation=20)
ax1.set_ylabel("Mean Latency (µs)")
ax1.set_title("Latency — 256ch × 10kHz (10 MB/s)")

ax2.bar(x, thrus, 0.5, color=colors, alpha=0.85)
for i, t in enumerate(thrus):
    ax2.text(i, t + 0.1, f"{t:.1f}", ha="center", fontsize=9, fontweight="bold")
ax2.set_xticks(x)
ax2.set_xticklabels(codecs, rotation=20)
ax2.set_ylabel("Data Rate (MB/s)")
ax2.set_title("Throughput — 256ch × 10kHz")

fig.suptitle("High-Throughput Codec Comparison — 256ch × 10kHz float32", fontsize=14, fontweight="bold", y=1.02)
fig.tight_layout()
fig.savefig(os.path.join(OUT, "06_high_throughput_codecs.png"), dpi=150, bbox_inches="tight")
plt.close(fig)
print("  ✓ 06_high_throughput_codecs.png")

# ── Chart 7: Zero Loss Summary ───────────────────────────────────────

fig, ax = plt.subplots(figsize=(10, 4))
all_losses = [r["loss_pct"] for r in rows]
all_labels = [f"{r['codec']}\n{int(r['channels'])}ch×{int(r['srate'])}Hz" for r in rows]

colors_all = ["#4CAF50" if l == 0.0 else "#F44336" for l in all_losses]
ax.bar(range(len(all_losses)), [1]*len(all_losses), color=colors_all, alpha=0.8, edgecolor="white", linewidth=0.5)

ax.set_xlim(-0.5, len(all_losses)-0.5)
ax.set_ylim(0, 1.2)
ax.set_yticks([])
ax.set_xticks([])
ax.set_title(f"Zero Data Loss — All {len(rows)} Benchmark Runs", fontsize=14, fontweight="bold")

n_pass = sum(1 for l in all_losses if l == 0.0)
n_fail = len(all_losses) - n_pass
ax.text(len(all_losses)/2, 0.5, f"PASS: {n_pass}/{len(all_losses)} runs with 0.00% loss",
        ha="center", va="center", fontsize=16, fontweight="bold", color="#2E7D32")
if n_fail > 0:
    ax.text(len(all_losses)/2, 0.2, f"FAIL: {n_fail} runs with loss",
            ha="center", va="center", fontsize=12, color="#C62828")

fig.tight_layout()
fig.savefig(os.path.join(OUT, "07_zero_loss_summary.png"), dpi=150)
plt.close(fig)
print("  ✓ 07_zero_loss_summary.png")

# ── Chart 8: delta-lz4 vs none scaling ────────────────────────────────

fig, ax = plt.subplots(figsize=(9, 5))
data_none = select(rows, codec="none", srate=2000.0, format="float32")
data_delta = select(rows, codec="delta-lz4", srate=2000.0, format="float32")
data_none.sort(key=lambda r: r["channels"])
data_delta.sort(key=lambda r: r["channels"])

# Only channels present in both
common_chs = sorted(set(int(r["channels"]) for r in data_none) & set(int(r["channels"]) for r in data_delta))
none_means = {int(r["channels"]): r["lat_mean_us"] for r in data_none}
delta_means = {int(r["channels"]): r["lat_mean_us"] for r in data_delta}

chs = common_chs
nm = [none_means[c] for c in chs]
dm = [delta_means[c] for c in chs]

x = np.arange(len(chs))
w = 0.35
ax.bar(x - w/2, nm, w, color=COLORS["none"], alpha=0.85, label="none")
ax.bar(x + w/2, dm, w, color=COLORS["delta-lz4"], alpha=0.85, label="delta-lz4")

for i, (n, d) in enumerate(zip(nm, dm)):
    ax.text(i - w/2, n + 0.3, f"{n:.1f}", ha="center", fontsize=8)
    ax.text(i + w/2, d + 0.3, f"{d:.1f}", ha="center", fontsize=8)

ax.set_xticks(x)
ax.set_xticklabels([str(c) for c in chs])
ax.set_xlabel("Channels per sample")
ax.set_ylabel("Mean Latency (µs)")
ax.set_title("delta-lz4 vs none — 2kHz float32")
ax.legend()
fig.tight_layout()
fig.savefig(os.path.join(OUT, "08_delta_lz4_vs_none.png"), dpi=150)
plt.close(fig)
print("  ✓ 08_delta_lz4_vs_none.png")

print(f"\nAll charts saved to {OUT}/")
