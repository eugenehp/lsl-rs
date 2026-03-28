//! Criterion benchmarks for sample serialization/deserialization.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rlsl::sample::Sample;
use rlsl::types::ChannelFormat;
use std::io::Cursor;

fn bench_serialize_110(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_110");

    let configs = vec![
        ("float32_8ch", ChannelFormat::Float32, 8),
        ("float32_32ch", ChannelFormat::Float32, 32),
        ("float32_128ch", ChannelFormat::Float32, 128),
        ("double64_8ch", ChannelFormat::Double64, 8),
        ("int16_8ch", ChannelFormat::Int16, 8),
        ("int8_8ch", ChannelFormat::Int8, 8),
        ("string_4ch", ChannelFormat::String, 4),
    ];

    for (name, fmt, nch) in &configs {
        let mut sample = Sample::new(*fmt, *nch, 0.0);
        sample.assign_test_pattern(0);

        group.bench_with_input(BenchmarkId::new("serialize", name), &sample, |b, s| {
            let mut buf = Vec::with_capacity(1024);
            b.iter(|| {
                buf.clear();
                s.serialize_110(black_box(&mut buf));
                black_box(&buf);
            });
        });
    }
    group.finish();
}

fn bench_deserialize_110(c: &mut Criterion) {
    let mut group = c.benchmark_group("deserialize_110");

    let configs: Vec<(&str, ChannelFormat, u32)> = vec![
        ("float32_8ch", ChannelFormat::Float32, 8),
        ("float32_32ch", ChannelFormat::Float32, 32),
        ("float32_128ch", ChannelFormat::Float32, 128),
        ("double64_8ch", ChannelFormat::Double64, 8),
        ("int16_8ch", ChannelFormat::Int16, 8),
        ("string_4ch", ChannelFormat::String, 4),
    ];

    for (name, fmt, nch) in &configs {
        let mut sample = Sample::new(*fmt, *nch, 0.0);
        sample.assign_test_pattern(0);
        let mut buf = Vec::new();
        sample.serialize_110(&mut buf);

        group.bench_with_input(
            BenchmarkId::new("deserialize", name),
            &(buf.clone(), *fmt, *nch),
            |b, (data, fmt, nch)| {
                b.iter(|| {
                    let mut cursor = Cursor::new(black_box(data.as_slice()));
                    let s = Sample::deserialize_110(&mut cursor, *fmt, *nch).unwrap();
                    black_box(s);
                });
            },
        );
    }
    group.finish();
}

fn bench_serialize_100(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialize_100");

    let mut sample = Sample::new(ChannelFormat::Float32, 8, 0.0);
    sample.assign_test_pattern(0);

    group.bench_function("float32_8ch", |b| {
        let mut buf = Vec::with_capacity(256);
        b.iter(|| {
            buf.clear();
            sample.serialize_100(black_box(&mut buf));
            black_box(&buf);
        });
    });
    group.finish();
}

fn bench_assign_retrieve(c: &mut Criterion) {
    let mut group = c.benchmark_group("assign_retrieve");

    let data_f32: Vec<f32> = (0..128).map(|i| i as f32).collect();
    let data_f64: Vec<f64> = (0..128).map(|i| i as f64).collect();

    group.bench_function("assign_f32_128ch", |b| {
        let mut sample = Sample::new(ChannelFormat::Float32, 128, 0.0);
        b.iter(|| {
            sample.assign_f32(black_box(&data_f32));
        });
    });

    group.bench_function("retrieve_f32_128ch", |b| {
        let mut sample = Sample::new(ChannelFormat::Float32, 128, 0.0);
        sample.assign_f32(&data_f32);
        let mut out = vec![0.0f32; 128];
        b.iter(|| {
            sample.retrieve_f32(black_box(&mut out));
            black_box(&out);
        });
    });

    group.bench_function("assign_f64_128ch", |b| {
        let mut sample = Sample::new(ChannelFormat::Double64, 128, 0.0);
        b.iter(|| {
            sample.assign_f64(black_box(&data_f64));
        });
    });

    group.bench_function("raw_roundtrip_128ch", |b| {
        let mut sample = Sample::new(ChannelFormat::Float32, 128, 0.0);
        sample.assign_f32(&data_f32);
        b.iter(|| {
            let raw = sample.retrieve_raw();
            let mut s2 = Sample::new(ChannelFormat::Float32, 128, 0.0);
            s2.assign_raw(black_box(&raw));
            black_box(s2);
        });
    });

    group.finish();
}

fn bench_xml_dom(c: &mut Criterion) {
    use rlsl::xml_dom::XmlNode;

    let mut group = c.benchmark_group("xml_dom");

    group.bench_function("build_channel_tree_32ch", |b| {
        b.iter(|| {
            let root = XmlNode::new("desc");
            let channels = root.append_child("channels");
            for i in 0..32 {
                let ch = channels.append_child("channel");
                ch.append_child_value("label", &format!("Ch{}", i));
                ch.append_child_value("unit", "uV");
                ch.append_child_value("type", "EEG");
            }
            black_box(root.to_xml());
        });
    });

    group.bench_function("navigate_32ch", |b| {
        let root = XmlNode::new("desc");
        let channels = root.append_child("channels");
        for i in 0..32 {
            let ch = channels.append_child("channel");
            ch.append_child_value("label", &format!("Ch{}", i));
        }

        b.iter(|| {
            let channels = root.child("channels");
            let mut ch = channels.first_child();
            let mut count = 0;
            while !ch.is_empty() {
                let _ = black_box(ch.child_value("label"));
                ch = ch.next_sibling();
                count += 1;
            }
            black_box(count);
        });
    });

    group.bench_function("deep_clone_32ch", |b| {
        let root = XmlNode::new("desc");
        let channels = root.append_child("channels");
        for i in 0..32 {
            let ch = channels.append_child("channel");
            ch.append_child_value("label", &format!("Ch{}", i));
            ch.append_child_value("unit", "uV");
        }

        b.iter(|| {
            black_box(root.deep_clone());
        });
    });

    group.bench_function("to_xml_32ch", |b| {
        let root = XmlNode::new("desc");
        let channels = root.append_child("channels");
        for i in 0..32 {
            let ch = channels.append_child("channel");
            ch.append_child_value("label", &format!("Ch{}", i));
            ch.append_child_value("unit", "uV");
        }

        b.iter(|| {
            black_box(root.to_xml());
        });
    });

    group.finish();
}

fn bench_stream_info(c: &mut Criterion) {
    use rlsl::stream_info::StreamInfo;

    let mut group = c.benchmark_group("stream_info");

    group.bench_function("to_shortinfo_message", |b| {
        let info = StreamInfo::new("Bench", "EEG", 32, 1000.0, ChannelFormat::Float32, "src1");
        b.iter(|| {
            black_box(info.to_shortinfo_message());
        });
    });

    group.bench_function("from_shortinfo_message", |b| {
        let info = StreamInfo::new("Bench", "EEG", 32, 1000.0, ChannelFormat::Float32, "src1");
        let xml = info.to_shortinfo_message();
        b.iter(|| {
            black_box(StreamInfo::from_shortinfo_message(black_box(&xml)));
        });
    });

    group.bench_function("matches_query_simple", |b| {
        let info = StreamInfo::new("MyEEG", "EEG", 8, 250.0, ChannelFormat::Float32, "src1");
        b.iter(|| {
            black_box(info.matches_query(black_box("name='MyEEG' and type='EEG'")));
        });
    });

    group.bench_function("matches_query_complex", |b| {
        let info = StreamInfo::new("MyEEG", "EEG", 8, 250.0, ChannelFormat::Float32, "src1");
        b.iter(|| {
            black_box(info.matches_query(black_box(
                "starts-with(name,'My') and channel_count>4 and not(type='Markers') or name='Other'",
            )));
        });
    });

    group.finish();
}

fn bench_postproc(c: &mut Criterion) {
    use rlsl::postproc::TimestampPostProcessor;
    use rlsl::types::*;

    let mut group = c.benchmark_group("postproc");

    group.bench_function("dejitter_250hz", |b| {
        let mut pp = TimestampPostProcessor::new(PROC_DEJITTER, 250.0, 90.0);
        let mut ts = 0.0;
        b.iter(|| {
            ts += 0.004;
            black_box(pp.process(black_box(ts)));
        });
    });

    group.bench_function("all_processors_250hz", |b| {
        let mut pp = TimestampPostProcessor::new(PROC_ALL, 250.0, 90.0);
        pp.set_clock_offset(0.1);
        let mut ts = 0.0;
        b.iter(|| {
            ts += 0.004;
            black_box(pp.process(black_box(ts)));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_serialize_110,
    bench_deserialize_110,
    bench_serialize_100,
    bench_assign_retrieve,
    bench_xml_dom,
    bench_stream_info,
    bench_postproc,
);
criterion_main!(benches);
