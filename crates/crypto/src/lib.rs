//! This is Spacedrive's `crypto` crate. It handles cryptographic operations
//! such as key hashing, encryption/decryption, key management and much more.
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::correctness)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::suspicious)]
#![warn(clippy::complexity)]
#![warn(clippy::nursery)]
#![warn(clippy::unwrap_used)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]
#![forbid(unsafe_code)]

pub mod crypto;
pub mod error;
pub mod fs;

pub mod keys;
pub mod primitives;
pub mod protected;
pub mod types;

#[cfg(feature = "headers")]
pub mod header;

#[cfg(feature = "headers")]
pub mod encoding;

// Re-export so they can be used elsewhere/cleaner `use` declarations
pub use self::error::{Error, Result};
pub use aead::Payload;
pub use protected::Protected;
pub use zeroize::Zeroize;
