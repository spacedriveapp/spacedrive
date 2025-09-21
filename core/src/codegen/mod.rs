//! Client generation system for type-safe API clients
//!
//! This module provides automated generation of type-safe Swift and TypeScript clients
//! by extracting JSON schemas from Rust types and operations.

pub mod extractor;
pub mod schema;

pub use extractor::*;
pub use schema::*;
