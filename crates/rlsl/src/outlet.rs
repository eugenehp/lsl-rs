//! StreamOutlet: publishes data on the lab network.

use crate::clock::local_clock;
use crate::sample::Sample;
use crate::send_buffer::SendBuffer;
use crate::stream_info::StreamInfo;
use crate::tcp_server::TcpServer;
use crate::types::*;
use crate::udp_server::UdpServer;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A stream outlet. Creates a discoverable stream on the network.
pub struct StreamOutlet {
    info: StreamInfo,
    send_buffer: Arc<SendBuffer>,
    shutdown: Arc<AtomicBool>,
    #[allow(dead_code)]
    chunk_size: i32,
}

impl StreamOutlet {
    /// Create a new stream outlet.
    pub fn new(info: &StreamInfo, chunk_size: i32, _max_buffered: i32) -> Self {
        let send_buffer = SendBuffer::new();

        // Set up the stream identity
        info.reset_uid();
        info.set_created_at(local_clock());
        info.set_session_id(&crate::config::CONFIG.session_id);
        info.set_hostname(
            &hostname::get()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );

        // Start TCP data server (IPv4 + IPv6)
        let tcp = TcpServer::start(info.clone(), send_buffer.clone(), chunk_size);
        info.set_v4data_port(tcp.v4_port);
        info.set_v6data_port(tcp.v6_port);

        // Start UDP service (time sync, IPv4 + IPv6)
        let (v4_svc_port, v6_svc_port) =
            UdpServer::start_unicast(info.clone(), tcp.shutdown.clone());
        info.set_v4service_port(v4_svc_port);
        info.set_v6service_port(v6_svc_port);

        // Start multicast responders (IPv4 + IPv6 groups)
        UdpServer::start_multicast(info.clone(), tcp.shutdown.clone());
        let tcp_shutdown = tcp.shutdown;

        StreamOutlet {
            info: info.clone(),
            send_buffer,
            shutdown: tcp_shutdown,
            chunk_size,
        }
    }

    /// Get the stream info
    pub fn info(&self) -> &StreamInfo {
        &self.info
    }

    /// Push a single float sample
    pub fn push_sample_f(&self, data: &[f32], timestamp: f64, pushthrough: bool) {
        let ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        let mut sample = Sample::new(self.info.channel_format(), self.info.channel_count(), ts);
        sample.assign_f32(data);
        sample.pushthrough = pushthrough;
        self.send_buffer.push_sample(sample);
    }

    /// Push a single double sample
    pub fn push_sample_d(&self, data: &[f64], timestamp: f64, pushthrough: bool) {
        let ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        let mut sample = Sample::new(self.info.channel_format(), self.info.channel_count(), ts);
        sample.assign_f64(data);
        sample.pushthrough = pushthrough;
        self.send_buffer.push_sample(sample);
    }

    pub fn push_sample_i32(&self, data: &[i32], timestamp: f64, pushthrough: bool) {
        let ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        let mut sample = Sample::new(self.info.channel_format(), self.info.channel_count(), ts);
        sample.assign_i32(data);
        sample.pushthrough = pushthrough;
        self.send_buffer.push_sample(sample);
    }

    pub fn push_sample_i16(&self, data: &[i16], timestamp: f64, pushthrough: bool) {
        let ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        let mut sample = Sample::new(self.info.channel_format(), self.info.channel_count(), ts);
        sample.assign_i16(data);
        sample.pushthrough = pushthrough;
        self.send_buffer.push_sample(sample);
    }

    pub fn push_sample_i64(&self, data: &[i64], timestamp: f64, pushthrough: bool) {
        let ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        let mut sample = Sample::new(self.info.channel_format(), self.info.channel_count(), ts);
        sample.assign_i64(data);
        sample.pushthrough = pushthrough;
        self.send_buffer.push_sample(sample);
    }

    pub fn push_sample_str(&self, data: &[String], timestamp: f64, pushthrough: bool) {
        let ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        let mut sample = Sample::new(self.info.channel_format(), self.info.channel_count(), ts);
        sample.assign_strings(data);
        sample.pushthrough = pushthrough;
        self.send_buffer.push_sample(sample);
    }

    pub fn push_sample_raw(&self, data: &[u8], timestamp: f64, pushthrough: bool) {
        let ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        let mut sample = Sample::new(self.info.channel_format(), self.info.channel_count(), ts);
        sample.assign_raw(data);
        sample.pushthrough = pushthrough;
        self.send_buffer.push_sample(sample);
    }

    /// Push a chunk of multiplexed float data
    pub fn push_chunk_f(&self, data: &[f32], timestamp: f64, pushthrough: bool) {
        let nch = self.info.channel_count() as usize;
        if nch == 0 {
            return;
        }
        let n_samples = data.len() / nch;
        let srate = self.info.nominal_srate();
        let mut ts = if timestamp == 0.0 {
            local_clock()
        } else {
            timestamp
        };
        if srate != IRREGULAR_RATE && n_samples > 1 {
            ts -= (n_samples - 1) as f64 / srate;
        }
        for i in 0..n_samples {
            let chunk = &data[i * nch..(i + 1) * nch];
            let is_last = i == n_samples - 1;
            let sample_ts = if i == 0 { ts } else { DEDUCED_TIMESTAMP };
            self.push_sample_f(chunk, sample_ts, pushthrough && is_last);
            if srate != IRREGULAR_RATE && i == 0 {
                // subsequent samples use deduced timestamp
            }
        }
    }

    /// Check if there are consumers
    pub fn have_consumers(&self) -> bool {
        self.send_buffer.have_consumers()
    }

    /// Wait for consumers
    pub fn wait_for_consumers(&self, timeout: f64) -> bool {
        self.send_buffer.wait_for_consumers(timeout)
    }
}

impl Drop for StreamOutlet {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        self.send_buffer.push_sentinel();
    }
}
