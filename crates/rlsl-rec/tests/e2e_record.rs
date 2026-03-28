//! End-to-end test: send data via outlet → record with Recording → verify output.

use arrow::array::RecordBatchReader;
use rlsl::prelude::*;
use rlsl::resolver;
use rlsl_rec::recording::{Recording, RecordingFormat};
use std::sync::atomic::Ordering;
use std::time::Duration;

// ── Helpers ──────────────────────────────────────────────────────────

async fn create_outlet(name: &str, src_id: &str) -> (StreamInfo, StreamOutlet) {
    let name = name.to_string();
    let src_id = src_id.to_string();
    let result = tokio::task::spawn_blocking(move || {
        let info = StreamInfo::new(&name, "EEG", 4, 250.0, ChannelFormat::Float32, &src_id);
        let outlet = StreamOutlet::new(&info, 0, 360);
        std::thread::sleep(Duration::from_secs(2));
        (info, outlet)
    })
    .await
    .unwrap();
    result
}

async fn resolve_stream(name: &str) -> StreamInfo {
    let resolved = tokio::task::spawn_blocking(move || resolver::resolve_all(3.0))
        .await
        .unwrap();
    resolved
        .into_iter()
        .find(|s| s.name() == name)
        .unwrap_or_else(|| panic!("Could not find stream '{}'", name))
}

fn push_samples(outlet: StreamOutlet, n: usize) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for i in 0..n {
            let sample = [
                i as f32 * 0.1,
                i as f32 * 0.2,
                i as f32 * 0.3,
                i as f32 * 0.4,
            ];
            outlet.push_sample_f(&sample, 0.0, true);
            std::thread::sleep(Duration::from_secs_f64(1.0 / 250.0));
        }
        std::thread::sleep(Duration::from_secs(2));
    })
}

// ── XDF test ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn e2e_xdf() {
    let (_info, outlet) = create_outlet("E2E_XDF", "e2e_xdf_src").await;
    let stream = resolve_stream("E2E_XDF").await;

    let path = "test_e2e_xdf.xdf";
    let rec = Recording::start_with_format(path, &[stream], RecordingFormat::Xdf)
        .expect("Failed to start XDF recording");

    let sender = push_samples(outlet, 500);
    tokio::time::sleep(Duration::from_secs(4)).await;

    let count = rec.state.sample_count.load(Ordering::Relaxed);
    println!("[XDF] Recorded {} samples", count);
    rec.stop().await;
    sender.join().unwrap();

    // Verify XDF file
    let data = std::fs::read(path).expect("XDF file not found");
    assert_eq!(&data[0..4], b"XDF:", "Missing XDF magic");
    assert!(data.len() > 100);

    let mut pos = 4;
    let (mut has_fh, mut has_sh, mut has_samp, mut has_sf) = (false, false, false, false);
    while pos < data.len() {
        let nlb = data[pos] as usize;
        pos += 1;
        if pos + nlb > data.len() {
            break;
        }
        let length = match nlb {
            1 => data[pos] as u64,
            4 => u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap()) as u64,
            8 => u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()),
            _ => break,
        };
        pos += nlb;
        if pos + 2 > data.len() {
            break;
        }
        let tag = u16::from_le_bytes(data[pos..pos + 2].try_into().unwrap());
        pos += length as usize;
        match tag {
            1 => has_fh = true,
            2 => has_sh = true,
            3 => has_samp = true,
            6 => has_sf = true,
            _ => {}
        }
    }
    assert!(has_fh, "Missing FileHeader");
    assert!(has_sh, "Missing StreamHeader");
    assert!(has_samp, "No sample chunks");
    assert!(has_sf, "Missing StreamFooter");
    assert!(count >= 200, "Only {} samples (expected ≥200)", count);
    println!(
        "✅ XDF e2e PASSED ({} samples, {} bytes)",
        count,
        data.len()
    );
    std::fs::remove_file(path).ok();
}

// ── Parquet test ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn e2e_parquet() {
    let (_info, outlet) = create_outlet("E2E_Parquet", "e2e_pq_src").await;
    let stream = resolve_stream("E2E_Parquet").await;

    let dir = "test_e2e_parquet";
    let rec = Recording::start_with_format(dir, &[stream], RecordingFormat::Parquet)
        .expect("Failed to start Parquet recording");

    let sender = push_samples(outlet, 500);
    tokio::time::sleep(Duration::from_secs(4)).await;

    let count = rec.state.sample_count.load(Ordering::Relaxed);
    println!("[Parquet] Recorded {} samples", count);
    rec.stop().await;
    sender.join().unwrap();

    // ── Verify directory structure ──
    assert!(
        std::path::Path::new(dir).is_dir(),
        "Output directory not created"
    );

    let entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();
    let filenames: Vec<String> = entries
        .iter()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    println!("[Parquet] Output files: {:?}", filenames);

    assert!(
        filenames.iter().any(|f| f == "metadata.json"),
        "Missing metadata.json sidecar"
    );

    let parquet_files: Vec<&String> = filenames
        .iter()
        .filter(|f| f.ends_with(".parquet"))
        .collect();
    assert!(!parquet_files.is_empty(), "No .parquet files found");

    // ── Validate JSON sidecar ──
    let sidecar_path = std::path::Path::new(dir).join("metadata.json");
    let sidecar_str = std::fs::read_to_string(&sidecar_path).expect("Cannot read metadata.json");
    let sidecar: serde_json::Value =
        serde_json::from_str(&sidecar_str).expect("Invalid JSON sidecar");
    println!(
        "[Parquet] Sidecar:\n{}",
        serde_json::to_string_pretty(&sidecar).unwrap()
    );

    assert!(sidecar["recording_start_unix"].as_f64().unwrap() > 0.0);
    let streams = sidecar["streams"]
        .as_array()
        .expect("streams should be array");
    assert_eq!(streams.len(), 1);
    let s = &streams[0];
    assert_eq!(s["name"].as_str().unwrap(), "E2E_Parquet");
    assert_eq!(s["type"].as_str().unwrap(), "EEG");
    assert_eq!(s["channel_count"].as_u64().unwrap(), 4);
    assert!((s["nominal_srate"].as_f64().unwrap() - 250.0).abs() < 0.01);
    assert_eq!(s["channel_format"].as_str().unwrap(), "float32");
    assert!(s["sample_count"].as_u64().unwrap() > 0);
    assert!(s["first_timestamp"].as_f64().unwrap() > 0.0);
    assert!(s["last_timestamp"].as_f64().unwrap() > s["first_timestamp"].as_f64().unwrap());
    assert!(s["header_xml"]
        .as_str()
        .unwrap()
        .contains("<name>E2E_Parquet</name>"));
    assert!(s["parquet_file"].as_str().unwrap().ends_with(".parquet"));
    println!("✓ Sidecar metadata validated");

    // ── Validate Parquet file ──
    let pq_path = std::path::Path::new(dir).join(s["parquet_file"].as_str().unwrap());
    let pq_file = std::fs::File::open(&pq_path).expect("Cannot open .parquet");
    let reader = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(pq_file)
        .expect("Invalid parquet file")
        .build()
        .expect("Cannot build reader");

    let schema = reader.schema();
    let field_names: Vec<&str> = schema
        .fields()
        .iter()
        .map(|f: &std::sync::Arc<arrow::datatypes::Field>| f.name().as_str())
        .collect();
    assert_eq!(field_names[0], "timestamp");
    assert_eq!(field_names.len(), 5);
    println!(
        "✓ Schema has {} columns: {:?}",
        field_names.len(),
        field_names
    );

    assert_eq!(
        *schema.field(0).data_type(),
        arrow::datatypes::DataType::Float64
    );
    for i in 1..5 {
        assert_eq!(
            *schema.field(i).data_type(),
            arrow::datatypes::DataType::Float32
        );
    }
    println!("✓ Column types correct");

    let mut total_rows = 0u64;
    let mut first_ts = f64::MAX;
    let mut last_ts = f64::MIN;
    for batch in reader {
        let batch = batch.expect("Error reading batch");
        let n = batch.num_rows();
        total_rows += n as u64;
        let ts_col = batch
            .column(0)
            .as_any()
            .downcast_ref::<arrow::array::Float64Array>()
            .expect("timestamp column not Float64");
        for i in 0..n {
            let t = ts_col.value(i);
            assert!(t > 0.0, "Timestamp should be positive, got {}", t);
            if t < first_ts {
                first_ts = t;
            }
            if t > last_ts {
                last_ts = t;
            }
        }
    }

    println!(
        "✓ Read {} rows, timestamps [{:.4} .. {:.4}]",
        total_rows, first_ts, last_ts
    );
    assert!(
        total_rows >= 200,
        "Only {} rows (expected ≥200)",
        total_rows
    );
    assert!(last_ts > first_ts);

    println!(
        "\n✅ Parquet e2e PASSED ({} samples recorded, {} rows in parquet)",
        count, total_rows
    );
    std::fs::remove_dir_all(dir).ok();
}
