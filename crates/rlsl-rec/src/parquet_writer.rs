//! Parquet (Arrow) file writer for LSL recordings.
//!
//! Creates one Parquet file per stream plus a JSON sidecar that describes
//! all streams, their metadata, clock offsets, and recording parameters.
//!
//! File layout for a recording with basename `recording_123`:
//!   recording_123/
//!     metadata.json           — sidecar with full stream descriptions
//!     stream_1_EEG.parquet    — columnar: timestamp, ch0, ch1, …
//!     stream_2_Markers.parquet
//!     …

use rlsl::stream_info::StreamInfo;
use rlsl::types::ChannelFormat;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

use exg::xdf::NumericSample;

// ── Sidecar types ────────────────────────────────────────────────────

/// JSON-serialisable metadata for the whole recording.
#[derive(serde::Serialize)]
struct RecordingSidecar {
    /// ISO-8601-ish recording start time (Unix seconds)
    recording_start_unix: f64,
    streams: Vec<StreamSidecar>,
}

/// Per-stream metadata written to the JSON sidecar.
#[derive(serde::Serialize)]
struct StreamSidecar {
    stream_id: u32,
    name: String,
    #[serde(rename = "type")]
    type_: String,
    channel_count: u32,
    nominal_srate: f64,
    channel_format: String,
    source_id: String,
    hostname: String,
    uid: String,
    session_id: String,
    created_at: f64,
    /// Full XML header from LSL (preserves <desc> metadata)
    header_xml: String,
    parquet_file: String,
    /// Channel labels extracted from <desc> if available
    channel_labels: Vec<String>,
    /// Clock offset measurements [(collection_time, offset_value), …]
    clock_offsets: Vec<(f64, f64)>,
    first_timestamp: f64,
    last_timestamp: f64,
    sample_count: u64,
}

// ── Per-stream Parquet file handle ───────────────────────────────────

struct StreamWriter {
    writer: ArrowWriter<fs::File>,
    schema: arrow::datatypes::SchemaRef,
    format: ChannelFormat,
    nch: usize,
    parquet_filename: String,
    /// Buffered rows before flushing to a row group
    ts_buf: Vec<f64>,
    data_buf: Vec<f64>, // everything upcast to f64 for simplicity
    buf_capacity: usize,
}

impl StreamWriter {
    fn new(
        dir: &Path,
        stream_id: u32,
        info: &StreamInfo,
        channel_labels: &[String],
    ) -> anyhow::Result<Self> {
        let nch = info.channel_count() as usize;
        let safe_name: String = info
            .name()
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let parquet_filename = format!("stream_{}_{}.parquet", stream_id, safe_name);
        let path = dir.join(&parquet_filename);

        // Build Arrow schema: timestamp + one column per channel
        let mut fields = vec![Field::new("timestamp", DataType::Float64, false)];
        for i in 0..nch {
            let label = if i < channel_labels.len() && !channel_labels[i].is_empty() {
                channel_labels[i].clone()
            } else {
                format!("ch{}", i)
            };
            let dt = arrow_dtype(info.channel_format());
            fields.push(Field::new(label, dt, false));
        }
        let schema = std::sync::Arc::new(Schema::new(fields));

        let file = fs::File::create(&path)?;
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(Default::default()))
            .build();
        let writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;

        Ok(StreamWriter {
            writer,
            schema,
            format: info.channel_format(),
            nch,
            parquet_filename,
            ts_buf: Vec::with_capacity(8192),
            data_buf: Vec::with_capacity(8192 * nch),
            buf_capacity: 8192,
        })
    }

    /// Append samples. `data` is flat [s0_ch0, s0_ch1, …, s1_ch0, …].
    fn append<T: NumericSample + ToF64>(
        &mut self,
        timestamps: &[f64],
        data: &[T],
        nch: u32,
    ) -> anyhow::Result<()> {
        let n = timestamps.len();
        assert_eq!(data.len(), n * nch as usize);
        for i in 0..n {
            self.ts_buf.push(timestamps[i]);
            for j in 0..nch as usize {
                self.data_buf.push(data[i * nch as usize + j].to_f64());
            }
        }
        if self.ts_buf.len() >= self.buf_capacity {
            self.flush_buffer()?;
        }
        Ok(())
    }

    fn flush_buffer(&mut self) -> anyhow::Result<()> {
        let n = self.ts_buf.len();
        if n == 0 {
            return Ok(());
        }
        let nch = self.nch;

        let ts_array = Float64Array::from(std::mem::take(&mut self.ts_buf));
        let data = std::mem::take(&mut self.data_buf);

        let mut columns: Vec<ArrayRef> = Vec::with_capacity(1 + nch);
        columns.push(std::sync::Arc::new(ts_array));

        for ch in 0..nch {
            let col_data: Vec<f64> = (0..n).map(|s| data[s * nch + ch]).collect();
            let array: ArrayRef = match self.format {
                ChannelFormat::Float32 => std::sync::Arc::new(Float32Array::from(
                    col_data.iter().map(|&v| v as f32).collect::<Vec<f32>>(),
                )),
                ChannelFormat::Double64 => std::sync::Arc::new(Float64Array::from(col_data)),
                ChannelFormat::Int16 => std::sync::Arc::new(Int16Array::from(
                    col_data.iter().map(|&v| v as i16).collect::<Vec<i16>>(),
                )),
                ChannelFormat::Int32 => std::sync::Arc::new(Int32Array::from(
                    col_data.iter().map(|&v| v as i32).collect::<Vec<i32>>(),
                )),
                ChannelFormat::Int64 => std::sync::Arc::new(Int64Array::from(
                    col_data.iter().map(|&v| v as i64).collect::<Vec<i64>>(),
                )),
                _ => {
                    // fallback to f64
                    std::sync::Arc::new(Float64Array::from(col_data))
                }
            };
            columns.push(array);
        }

        let batch = RecordBatch::try_new(self.schema.clone(), columns)?;
        self.writer.write(&batch)?;
        Ok(())
    }

    fn close(mut self) -> anyhow::Result<()> {
        self.flush_buffer()?;
        self.writer.close()?;
        Ok(())
    }
}

// ── Public ParquetRecordingWriter ────────────────────────────────────

/// Thread-safe Parquet recording writer. Mirrors the XdfWriter API.
pub struct ParquetRecordingWriter {
    dir: PathBuf,
    streams: Mutex<HashMap<u32, StreamWriter>>,
    sidecar: Mutex<RecordingSidecar>,
    infos: Mutex<HashMap<u32, StreamInfo>>,
}

impl ParquetRecordingWriter {
    /// Create the output directory and initialise.
    pub fn new(dir_path: &str) -> anyhow::Result<Self> {
        let dir = PathBuf::from(dir_path);
        fs::create_dir_all(&dir)?;
        let start = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        Ok(ParquetRecordingWriter {
            dir,
            streams: Mutex::new(HashMap::new()),
            sidecar: Mutex::new(RecordingSidecar {
                recording_start_unix: start,
                streams: Vec::new(),
            }),
            infos: Mutex::new(HashMap::new()),
        })
    }

    /// Register a stream and create its Parquet file. Call before writing samples.
    pub fn write_stream_header(
        &self,
        stream_id: u32,
        info: &StreamInfo,
        header_xml: &str,
    ) -> anyhow::Result<()> {
        let channel_labels = extract_channel_labels(info);
        let sw = StreamWriter::new(&self.dir, stream_id, info, &channel_labels)?;

        let mut sidecar = self.sidecar.lock().unwrap();
        sidecar.streams.push(StreamSidecar {
            stream_id,
            name: info.name(),
            type_: info.type_(),
            channel_count: info.channel_count(),
            nominal_srate: info.nominal_srate(),
            channel_format: info.channel_format().as_str().to_string(),
            source_id: info.source_id(),
            hostname: info.hostname(),
            uid: info.uid(),
            session_id: info.session_id(),
            created_at: info.created_at(),
            header_xml: header_xml.to_string(),
            parquet_file: sw.parquet_filename.clone(),
            channel_labels,
            clock_offsets: Vec::new(),
            first_timestamp: 0.0,
            last_timestamp: 0.0,
            sample_count: 0,
        });
        drop(sidecar);

        self.infos.lock().unwrap().insert(stream_id, info.clone());
        self.streams.lock().unwrap().insert(stream_id, sw);
        Ok(())
    }

    /// Write numeric samples for a stream.
    pub fn write_samples_numeric<T: NumericSample + ToF64>(
        &self,
        stream_id: u32,
        timestamps: &[f64],
        data: &[T],
        n_channels: u32,
    ) -> anyhow::Result<()> {
        if timestamps.is_empty() {
            return Ok(());
        }
        let mut streams = self.streams.lock().unwrap();
        if let Some(sw) = streams.get_mut(&stream_id) {
            sw.append(timestamps, data, n_channels)?;
        }
        Ok(())
    }

    /// Record a clock offset measurement.
    pub fn write_clock_offset(
        &self,
        stream_id: u32,
        collection_time: f64,
        offset_value: f64,
    ) -> anyhow::Result<()> {
        let mut sidecar = self.sidecar.lock().unwrap();
        if let Some(entry) = sidecar
            .streams
            .iter_mut()
            .find(|s| s.stream_id == stream_id)
        {
            entry.clock_offsets.push((collection_time, offset_value));
        }
        Ok(())
    }

    /// Write stream footer (updates sidecar with final stats).
    pub fn write_stream_footer(
        &self,
        stream_id: u32,
        first_ts: f64,
        last_ts: f64,
        sample_count: u64,
    ) -> anyhow::Result<()> {
        let mut sidecar = self.sidecar.lock().unwrap();
        if let Some(entry) = sidecar
            .streams
            .iter_mut()
            .find(|s| s.stream_id == stream_id)
        {
            entry.first_timestamp = first_ts;
            entry.last_timestamp = last_ts;
            entry.sample_count = sample_count;
        }
        Ok(())
    }

    /// Finalize: close all Parquet files and write the JSON sidecar.
    pub fn close(self) -> anyhow::Result<()> {
        // Close all stream writers
        let streams = self.streams.into_inner().unwrap();
        for (_, sw) in streams {
            sw.close()?;
        }
        // Write sidecar JSON
        let sidecar = self.sidecar.into_inner().unwrap();
        let json = serde_json::to_string_pretty(&sidecar)?;
        let path = self.dir.join("metadata.json");
        let mut f = fs::File::create(&path)?;
        f.write_all(json.as_bytes())?;
        Ok(())
    }

    /// Convenience: directory path.
    pub fn dir_path(&self) -> &Path {
        &self.dir
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn arrow_dtype(fmt: ChannelFormat) -> DataType {
    match fmt {
        ChannelFormat::Float32 => DataType::Float32,
        ChannelFormat::Double64 => DataType::Float64,
        ChannelFormat::Int16 => DataType::Int16,
        ChannelFormat::Int32 => DataType::Int32,
        ChannelFormat::Int64 => DataType::Int64,
        ChannelFormat::Int8 => DataType::Int8,
        _ => DataType::Float64, // fallback
    }
}

/// Extract channel labels from the <desc><channels><channel><label> hierarchy.
fn extract_channel_labels(info: &StreamInfo) -> Vec<String> {
    let desc = info.desc();
    let channels_node = desc.child("channels");
    if channels_node.is_empty() {
        return Vec::new();
    }
    let mut labels = Vec::new();
    let mut ch = channels_node.child("channel");
    while !ch.is_empty() {
        let label = ch.child_value("label");
        labels.push(if label.is_empty() {
            format!("ch{}", labels.len())
        } else {
            label
        });
        ch = ch.next_sibling_named("channel");
    }
    labels
}

/// Trait for converting sample values to f64 for intermediate buffering.
pub trait ToF64 {
    fn to_f64(&self) -> f64;
}

impl ToF64 for f32 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}
impl ToF64 for f64 {
    fn to_f64(&self) -> f64 {
        *self
    }
}
impl ToF64 for i16 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}
impl ToF64 for i32 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}
impl ToF64 for i64 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}
impl ToF64 for i8 {
    fn to_f64(&self) -> f64 {
        *self as f64
    }
}
