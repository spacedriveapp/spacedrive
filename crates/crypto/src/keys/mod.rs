//! This module contains all key and hashing related functions.

mod hashing;
pub use hashing::Hasher;

#[cfg(feature = "os-keyrings")]
pub mod keyring;
