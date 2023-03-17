//! This module will contains all header related functions.
//!
//! It handles serialisation, deserialisation, AAD, keyslots and metadata, preview media.
pub mod encoding;
pub(crate) mod file;
pub mod schema;

pub use file::{FileHeader, FileHeaderVersion, HeaderBundle, HeaderObjectName};
