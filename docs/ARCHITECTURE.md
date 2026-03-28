# Architecture

This document describes the internal architecture of lsl-rs, a pure-Rust
implementation of the Lab Streaming Layer (LSL) protocol.

## Crate Dependency Graph

```
                    ┌──────────┐
                    │ lsl-core │  ← Pure Rust library (no C deps)
                    └────┬─────┘
          ┌──────────┬───┼───────┬──────────┬───────────┐
          │          │   │       │          │           │
     ┌────▼──┐  ┌───▼──┐│  ┌───▼───┐ ┌───▼────┐  ┌──▼──────┐
     │lsl-sys│  │lsl-py││  │lsl-gen│ │lsl-wasm│  │lsl-bench│
     │(cdylib)│  │(PyO3) ││  │(bin)  │ │(bridge)│  │(bin)    │
     └───────┘  └──────┘│  └───────┘ └────────┘  └─────────┘
                         │
              ┌──────────┼──────────┐
              │          │          │
         ┌────▼───┐  ┌──▼───┐  ┌──▼────────┐
         │lsl-rec │  │ exg  │  │lsl-convert│
         │(lib+bin)│  │(XDF) │  │(bin)      │
         └────┬───┘  └──────┘  └───────────┘
              │
         ┌────▼──────┐
         │lsl-rec-gui│
         │(eGUI)     │
         └───────────┘
```

## Core Library (`lsl-core`)

### Module Layout

```
lsl-core/src/
├── lib.rs              # Crate root, shared tokio runtime, prelude
├── types.rs            # ChannelFormat, ErrorCode, protocol constants
├── clock.rs            # Monotonic local_clock() (std::time::Instant)
├── config.rs           # lsl_api.cfg loading (env → file → defaults)
├── stream_info.rs      # StreamInfo metadata + XPath query matching
├── xml_dom.rs          # Mutable XML tree (Arc<Mutex<NodeData>>)
├── sample.rs           # Typed samples + protocol 1.00/1.10 serde
├── send_buffer.rs      # SPMC broadcast buffer (crossbeam channels)
├── outlet.rs           # StreamOutlet (TCP+UDP servers)
├── inlet.rs            # StreamInlet (TCP client + recovery)
├── resolver.rs         # UDP multicast/broadcast discovery
├── tcp_server.rs       # TCP data feed (per-connection async tasks)
├── udp_server.rs       # UDP discovery + time-sync responder
├── time_receiver.rs    # NTP-like clock offset estimation
├── postproc.rs         # Dejitter, clocksync, monotonize filters
└── signal_quality.rs   # SNR, jitter, dropout metrics
```

### Threading Model

```
┌─────────────────────────────────────────────────────────┐
│                 Shared Tokio Runtime                     │
│                 (4 worker threads)                       │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ TCP accept   │  │ UDP multicast│  │ UDP time      │  │
│  │ loop (per    │  │ responder    │  │ responder     │  │
│  │ outlet)      │  │              │  │              │  │
│  └──────┬───────┘  └──────────────┘  └──────────────┘  │
│         │                                               │
│  ┌──────▼───────┐                                      │
│  │ TCP session  │  (spawned per connected inlet)       │
│  │ async task   │                                      │
│  └──────────────┘                                      │
└─────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────┐
│              Inlet Receiver Thread                   │
│  (dedicated std::thread per inlet, own tokio rt)   │
│                                                     │
│  ┌──────────────┐    ┌──────────────┐              │
│  │ TCP connect  │───▶│ Sample       │──▶ crossbeam │
│  │ + read loop  │    │ deserialize  │    channel    │
│  └──────────────┘    └──────────────┘              │
└────────────────────────────────────────────────────┘
```

### Protocol Flow

#### Stream Discovery (UDP)

```
 Resolver                         Outlet (UDP Server)
    │                                    │
    │──── UDP multicast ────────────────▶│
    │     "LSL:shortinfo\r\n             │
    │      query\r\n                     │
    │      return_after version\r\n"     │
    │                                    │
    │◀─── UDP unicast reply ─────────────│
    │     (shortinfo XML)                │
    │                                    │
```

#### Data Streaming (TCP)

```
 Inlet                           Outlet (TCP Server)
    │                                    │
    │──── TCP connect ──────────────────▶│
    │                                    │
    │──── "LSL:streamfeed\r\n" ────────▶│
    │     "protocol_version\r\n"         │
    │     "max_buflen 360\r\n"           │
    │     "\r\n"                         │
    │                                    │
    │◀─── fullinfo XML ─────────────────│
    │     "\r\n"                         │
    │                                    │
    │◀─── binary samples ───────────────│
    │     [tag][timestamp?][data]        │
    │     [tag][timestamp?][data]        │
    │     ...                            │
```

#### Sample Wire Format (Protocol 1.10)

```
Numeric sample:
┌─────────┬──────────────────┬──────────────────────────┐
│ tag (1B)│ timestamp (8B)?  │ channel data (N × fmt)   │
└─────────┴──────────────────┴──────────────────────────┘
  0x01 = deduced (no timestamp bytes)
  0x02 = transmitted (8-byte f64 follows)

String sample:
┌─────────┬──────────────────┬──────────────────────────┐
│ tag (1B)│ timestamp (8B)?  │ per-channel:             │
│         │                  │ [len_size][len][utf8...]  │
└─────────┴──────────────────┴──────────────────────────┘
  len_size: 1 → u8 len, 4 → u32 len, 8 → u64 len
```

#### Time Correction (UDP)

```
 Inlet                           Outlet (UDP Server)
    │                                    │
    │──── "LSL:timedata\r\n             │
    │      wave_id t0\r\n" ────────────▶│
    │                                    │
    │◀─── "wave_id t0 t1 t2\r\n" ──────│
    │                                    │
    │  offset = ((t1-t0) + (t2-t3)) / 2 │
```

### Concurrency Primitives

| Primitive | Usage |
|-----------|-------|
| `Arc<Mutex<T>>` (parking_lot) | StreamInfo shared state, XML DOM nodes |
| `crossbeam_channel` | SendBuffer → TCP sessions, Inlet sample queue |
| `AtomicBool` / `AtomicU32` | Outlet running flag, inlet state |
| `tokio::sync` | Async coordination in TCP/UDP servers |
| `once_cell::Lazy` | Global runtime, config, clock epoch |

### Configuration Cascade

```
Environment variables (LSL_MULTICAST_PORT, LSL_IPV6, ...)
        │
        ▼
Config file search:
  1. ./lsl_api.cfg
  2. ~/.lsl/lsl_api.cfg
  3. /etc/lsl_api/lsl_api.cfg
        │
        ▼
Built-in defaults (types.rs constants)
```

## Recording Pipeline (`lsl-rec`)

```
┌──────────┐    ┌───────────┐    ┌────────────┐    ┌──────────┐
│ Resolver  │───▶│ Recording │───▶│ XDF Writer │───▶│ .xdf     │
│ (discover)│    │ Engine    │    │ (exg crate)│    │ file     │
└──────────┘    │           │    └────────────┘    └──────────┘
                │           │    ┌────────────┐    ┌──────────┐
                │           │───▶│ Parquet    │───▶│ .parquet │
                │           │    │ Writer     │    │ files    │
                └───────────┘    └────────────┘    └──────────┘
                      │
                ┌─────▼─────┐
                │ TUI / GUI │
                │ (ratatui / │
                │  eGUI)     │
                └───────────┘
```

## WebSocket Bridge (`lsl-wasm`)

```
 Browser (WASM)              Bridge Server              LSL Network
┌────────────┐           ┌──────────────┐          ┌──────────────┐
│ lsl_wasm   │◀──WS────▶│  lsl-bridge  │◀──LSL──▶│  Outlets     │
│ (JS/WASM)  │  JSON     │  (tokio +    │  TCP     │  (any host)  │
│            │  frames   │  tungstenite)│          │              │
└────────────┘           └──────────────┘          └──────────────┘
```

## C ABI Layer (`lsl-sys`)

The `lsl-sys` crate exposes 162 `extern "C"` functions matching liblsl's API exactly.
Handles are opaque pointers to Rust objects stored in `Box`:

```rust
#[no_mangle]
pub extern "C" fn lsl_create_outlet(info: lsl_streaminfo, chunk_size: i32, max_buffered: i32) -> lsl_outlet {
    let info = unsafe { &*info }.clone();
    let outlet = StreamOutlet::new(&info, chunk_size, max_buffered);
    Box::into_raw(Box::new(outlet))
}

#[no_mangle]
pub extern "C" fn lsl_destroy_outlet(obj: lsl_outlet) {
    if !obj.is_null() {
        unsafe { let _ = Box::from_raw(obj); }
    }
}
```

## Python Bindings (`lsl-py`)

Uses PyO3 to wrap `lsl-core` types as Python classes:

```python
import pylsl

info = pylsl.StreamInfo("EEG", "EEG", 8, 250.0, pylsl.CF_FLOAT32, "src1")
outlet = pylsl.StreamOutlet(info)
outlet.push_sample([1.0] * 8)

# Inlet returns numpy arrays
inlet = pylsl.StreamInlet(streams[0])
chunk, timestamps = inlet.pull_chunk()  # → (ndarray, ndarray)
```

## Performance Characteristics

| Operation | Typical Latency |
|-----------|----------------|
| `push_sample` → `pull_sample` (in-process) | < 50 µs |
| `push_sample` → `pull_sample` (localhost TCP) | 100–500 µs |
| Sample serialization (8ch float32, proto 1.10) | < 1 µs |
| UDP discovery round-trip | 1–5 ms |
| Time correction probe | 5–50 ms |
| Stream recovery after disconnect | 100–500 ms |
