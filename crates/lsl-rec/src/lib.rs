//! `lsl-rec` library — shared recording engine and Parquet writer.
//!
//! The XDF writer and `NumericSample` trait live in the [`exg`] crate.

pub mod hdf5_writer;
pub mod markers;
pub mod parquet_writer;
pub mod recording;
