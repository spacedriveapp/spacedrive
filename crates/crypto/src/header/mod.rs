//! This module will contains all header related functions.
//!
//! It handles serialization, deserialization, AAD, keyslots and metadata, preview media.
pub(crate) mod file;
pub mod schema;

pub use file::{FileHeader, FileHeaderVersion, HeaderBundle, HeaderObjectName};
