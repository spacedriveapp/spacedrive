//! This module contains all key and hashing related functions.

mod hashing;
pub use hashing::Hasher;

#[cfg(all(feature = "keymanager", feature = "os-keyrings"))]
pub mod keymanager;

#[cfg(feature = "os-keyrings")]
pub mod keyring;
