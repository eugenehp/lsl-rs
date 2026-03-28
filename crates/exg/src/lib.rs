//! `exg` — exchange format library for LSL recordings.
//!
//! Provides the XDF file writer and the shared `NumericSample` trait used
//! by all recording backends (XDF, Parquet, …).

pub mod xdf_writer;

// Re-export commonly used items at crate root.
pub use xdf_writer::{NumericSample, XdfWriter};
