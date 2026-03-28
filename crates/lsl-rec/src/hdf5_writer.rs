//! HDF5 file writer for LSL recordings.
//!
//! Creates one HDF5 file per recording with groups per stream:
//!
//! ```text
//! recording.h5
//! ├── stream_1_EEG/
//! │   ├── timestamps   (dataset: float64[N])
//! │   ├── data         (dataset: float32[N × nch] or matching format)
//! │   └── .attrs       (name, type, srate, channel_format, hostname, uid, ...)
//! ├── stream_2_Markers/
//! │   └── ...
//! └── .attrs            (recording_start, session_id)
//! ```
//!
//! **Note**: This module is a stub that writes HDF5-compatible data using
//! a minimal binary layout. For full HDF5 support, add the `hdf5` crate
//! dependency. Currently we write per-stream Parquet files as a portable
//! fallback, since HDF5 requires a C library (libhdf5) to be installed.

// For now, HDF5 output is aliased to Parquet output with a `.h5` note.
// Full HDF5 can be added later with the `hdf5` crate when libhdf5 is available.

use crate::parquet_writer::ParquetRecordingWriter;

/// HDF5 writer is currently implemented as a Parquet writer with HDF5-style
/// directory layout. Install libhdf5 and the `hdf5` crate for native HDF5.
pub type Hdf5RecordingWriter = ParquetRecordingWriter;
