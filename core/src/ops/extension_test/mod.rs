//! Test operations for extension system validation
//!
//! These operations exist solely to test the WASM extension system.
//! They provide simple functionality that extensions can call to validate
//! the full WASM → Wire → Operation flow.

mod ping;

pub use ping::*;

