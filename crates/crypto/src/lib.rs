//! This is Spacedrive's `crypto` crate. It handles cryptographic operations
//! such as key hashing, encryption/decryption, key management and much more.
#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(clippy::correctness)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::suspicious)]
#![warn(clippy::nursery)]
#![warn(clippy::complexity)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::similar_names)]

pub mod crypto;
pub mod error;
pub mod fs;
pub mod header;
pub mod keys;
pub mod primitives;
pub mod protected;
pub mod types;

// Re-export so they can be used elsewhere/cleaner `use` declarations
pub use self::error::{Error, Result};
pub use aead::Payload;
pub use protected::Protected;
pub use zeroize::Zeroize;
