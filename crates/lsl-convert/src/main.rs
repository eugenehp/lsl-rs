//! `lsl-convert` — Offline format converter for LSL recordings.
//!
//! Usage:
//!   lsl-convert input.xdf --to parquet -o output_dir/
//!   lsl-convert input_dir/ --to csv -o output.csv     (Parquet dir → CSV)
//!   lsl-convert input.xdf --to csv -o output.csv
//!   lsl-convert input.xdf --info                       (print stream info)

use anyhow::{Context, Result};
use arrow::array::*;
use arrow::csv::WriterBuilder as CsvWriterBuilder;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::io::Write;
use std::sync::Arc;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: lsl-convert <input> [--to parquet|csv|info] [-o output]");
        std::process::exit(1);
    }

    let input = &args[1];
    let format = args
        .iter()
        .position(|a| a == "--to")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str())
        .unwrap_or("info");
    let output = args
        .iter()
        .position(|a| a == "-o")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.as_str());

    if format == "info" || args.contains(&"--info".to_string()) {
        return xdf_info(input);
    }

    match (input_type(input), format) {
        (InputType::Xdf, "parquet") => xdf_to_parquet(input, output.unwrap_or("output_parquet"))?,
        (InputType::Xdf, "csv") => xdf_to_csv(input, output.unwrap_or("output.csv"))?,
        (InputType::Parquet, "csv") => parquet_to_csv(input, output.unwrap_or("output.csv"))?,
        _ => anyhow::bail!("Unsupported conversion: {} → {}", input, format),
    }

    Ok(())
}

#[derive(Debug)]
enum InputType {
    Xdf,
    Parquet,
    Unknown,
}

fn input_type(path: &str) -> InputType {
    if path.ends_with(".xdf") {
        InputType::Xdf
    } else if path.ends_with(".parquet") || std::path::Path::new(path).is_dir() {
        InputType::Parquet
    } else {
        InputType::Unknown
    }
}

// ── XDF parsing (minimal) ────────────────────────────────────────────

struct XdfStream {
    stream_id: u32,
    name: String,
    nch: usize,
    srate: f64,
    timestamps: Vec<f64>,
    data: Vec<f64>, // flat: [s0ch0, s0ch1, ..., s1ch0, ...]
}

fn parse_xdf(path: &str) -> Result<Vec<XdfStream>> {
    let data = std::fs::read(path).context("Cannot read XDF file")?;
    anyhow::ensure!(&data[..4] == b"XDF:", "Not a valid XDF file");

    let mut pos = 4;
    let mut streams: std::collections::HashMap<u32, XdfStream> = std::collections::HashMap::new();

    while pos < data.len() {
        let nlb = *data.get(pos).unwrap_or(&0) as usize;
        pos += 1;
        if pos + nlb > data.len() {
            break;
        }
        let length = match nlb {
            1 => data[pos] as u64,
            4 => u32::from_le_bytes(data[pos..pos + 4].try_into()?) as u64,
            8 => u64::from_le_bytes(data[pos..pos + 8].try_into()?),
            _ => break,
        };
        pos += nlb;
        let chunk_start = pos;
        if pos + 2 > data.len() {
            break;
        }
        let tag = u16::from_le_bytes(data[pos..pos + 2].try_into()?);
        pos = chunk_start + length as usize;
        if pos > data.len() {
            break;
        }

        match tag {
            2 => {
                // StreamHeader
                let sid = u32::from_le_bytes(data[chunk_start + 2..chunk_start + 6].try_into()?);
                let xml =
                    std::str::from_utf8(&data[chunk_start + 6..chunk_start + length as usize])
                        .unwrap_or("");
                let name = extract_xml_tag(xml, "name").unwrap_or_default();
                let nch: usize = extract_xml_tag(xml, "channel_count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1);
                let srate: f64 = extract_xml_tag(xml, "nominal_srate")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                streams.insert(
                    sid,
                    XdfStream {
                        stream_id: sid,
                        name,
                        nch,
                        srate,
                        timestamps: vec![],
                        data: vec![],
                    },
                );
            }
            3 => {
                // Samples
                let sid = u32::from_le_bytes(data[chunk_start + 2..chunk_start + 6].try_into()?);
                if let Some(stream) = streams.get_mut(&sid) {
                    // Minimal f32 sample parsing (assumes float32 for simplicity)
                    let content = &data[chunk_start + 6..chunk_start + length as usize];
                    parse_samples_f32(content, stream);
                }
            }
            _ => {}
        }
    }

    Ok(streams.into_values().collect())
}

fn parse_samples_f32(content: &[u8], stream: &mut XdfStream) {
    let nch = stream.nch;
    let mut p = 0;
    // Read num_samples varlen
    if p >= content.len() {
        return;
    }
    let nlb = content[p] as usize;
    p += 1;
    if p + nlb > content.len() {
        return;
    }
    let n_samples = match nlb {
        1 => content[p] as usize,
        4 => u32::from_le_bytes(content[p..p + 4].try_into().unwrap_or([0; 4])) as usize,
        _ => return,
    };
    p += nlb;

    let mut last_ts = stream.timestamps.last().copied().unwrap_or(0.0);
    let interval = if stream.srate > 0.0 {
        1.0 / stream.srate
    } else {
        0.0
    };

    for _ in 0..n_samples {
        if p >= content.len() {
            break;
        }
        // Timestamp
        let ts_len = content[p] as usize;
        p += 1;
        let ts = if ts_len == 8 && p + 8 <= content.len() {
            let t = f64::from_le_bytes(content[p..p + 8].try_into().unwrap_or([0; 8]));
            p += 8;
            last_ts = t;
            t
        } else {
            last_ts += interval;
            last_ts
        };
        stream.timestamps.push(ts);

        // Channel values (float32)
        for _ in 0..nch {
            if p + 4 > content.len() {
                break;
            }
            let v = f32::from_le_bytes(content[p..p + 4].try_into().unwrap_or([0; 4]));
            stream.data.push(v as f64);
            p += 4;
        }
    }
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml.find(&close)?;
    Some(xml[start..end].to_string())
}

// ── Conversions ──────────────────────────────────────────────────────

fn xdf_info(path: &str) -> Result<()> {
    let streams = parse_xdf(path)?;
    eprintln!("XDF file: {}", path);
    eprintln!("{} stream(s):", streams.len());
    for s in &streams {
        let n = s.timestamps.len();
        let ts_range = if n > 0 {
            format!("[{:.4}..{:.4}]", s.timestamps[0], s.timestamps[n - 1])
        } else {
            "[]".into()
        };
        eprintln!(
            "  stream_id={}: name=\"{}\", {}ch, {}Hz, {} samples, ts {}",
            s.stream_id, s.name, s.nch, s.srate, n, ts_range
        );
    }
    Ok(())
}

fn xdf_to_parquet(input: &str, output_dir: &str) -> Result<()> {
    let streams = parse_xdf(input)?;
    std::fs::create_dir_all(output_dir)?;

    for s in &streams {
        let n = s.timestamps.len();
        if n == 0 {
            continue;
        }

        let mut fields = vec![Field::new("timestamp", DataType::Float64, false)];
        for ch in 0..s.nch {
            fields.push(Field::new(format!("ch{}", ch), DataType::Float64, false));
        }
        let schema = Arc::new(Schema::new(fields));

        let path = format!(
            "{}/stream_{}_{}.parquet",
            output_dir,
            s.stream_id,
            s.name
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect::<String>()
        );
        let file = std::fs::File::create(&path)?;
        let props = WriterProperties::builder()
            .set_compression(Compression::ZSTD(Default::default()))
            .build();
        let mut writer = ArrowWriter::try_new(file, schema.clone(), Some(props))?;

        let ts_arr = Float64Array::from(s.timestamps.clone());
        let mut cols: Vec<ArrayRef> = vec![Arc::new(ts_arr)];
        for ch in 0..s.nch {
            let col: Vec<f64> = (0..n).map(|i| s.data[i * s.nch + ch]).collect();
            cols.push(Arc::new(Float64Array::from(col)));
        }
        let batch = RecordBatch::try_new(schema, cols)?;
        writer.write(&batch)?;
        writer.close()?;
        eprintln!("  ✓ {} ({} samples)", path, n);
    }
    eprintln!("Done: {} → {}", input, output_dir);
    Ok(())
}

fn xdf_to_csv(input: &str, output: &str) -> Result<()> {
    let streams = parse_xdf(input)?;
    let mut file = std::fs::File::create(output)?;

    for s in &streams {
        let n = s.timestamps.len();
        if n == 0 {
            continue;
        }

        // Header
        write!(file, "stream,timestamp")?;
        for ch in 0..s.nch {
            write!(file, ",ch{}", ch)?;
        }
        writeln!(file)?;

        for i in 0..n {
            write!(file, "{},{:.9}", s.name, s.timestamps[i])?;
            for ch in 0..s.nch {
                write!(file, ",{}", s.data[i * s.nch + ch])?;
            }
            writeln!(file)?;
        }
    }
    eprintln!("Done: {} → {} ({} streams)", input, output, streams.len());
    Ok(())
}

fn parquet_to_csv(input: &str, output: &str) -> Result<()> {
    use arrow::array::RecordBatchReader;

    // Find .parquet files
    let paths: Vec<_> = if std::path::Path::new(input).is_dir() {
        std::fs::read_dir(input)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|x| x == "parquet")
                    .unwrap_or(false)
            })
            .map(|e| e.path())
            .collect()
    } else {
        vec![std::path::PathBuf::from(input)]
    };

    let mut out = std::fs::File::create(output)?;
    let mut first = true;

    for path in &paths {
        let file = std::fs::File::open(path)?;
        let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?
            .build()?;
        let schema = reader.schema();

        if first {
            // CSV header
            let names: Vec<&str> = schema.fields().iter().map(|f| f.name().as_str()).collect();
            writeln!(out, "{}", names.join(","))?;
            first = false;
        }

        let mut csv_writer = CsvWriterBuilder::new().with_header(false).build(&mut out);
        for batch in reader {
            csv_writer.write(&batch?)?;
        }
    }
    eprintln!("Done: {} → {}", input, output);
    Ok(())
}
