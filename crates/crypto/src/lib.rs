#![doc = include_str!("../README.md")]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::correctness)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::suspicious)]
#![warn(clippy::complexity)]
#![warn(clippy::nursery)]
#![warn(clippy::unwrap_used)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![warn(unused_qualifications, rust_2018_idioms)]
#![forbid(unsafe_code)]

pub mod crypto;
pub mod ct;
pub mod error;
// pub mod keys;
pub mod hashing;
pub mod primitives;
pub mod protected;
pub mod types;
pub mod utils;

#[cfg(feature = "sys")]
pub mod sys;

#[cfg(feature = "encoding")]
pub mod encoding;

// #[cfg(feature = "async")]
// pub mod fs;

// #[cfg(feature = "encoding")]
// pub mod header;

// Re-export so they can be used elsewhere/cleaner `use` declarations
pub use self::error::{Error, Result};
pub use aead::Payload;
pub use protected::Protected;
pub use zeroize::Zeroize;
