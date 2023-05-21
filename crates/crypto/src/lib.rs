#![doc = include_str!("../README.md")]
#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	clippy::expect_used,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::as_conversions,
	clippy::dbg_macro
)]
#![forbid(unsafe_code)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]

pub mod crypto;
pub mod ct;
pub mod error;
pub mod hashing;
pub mod primitives;
pub mod protected;
pub mod types;
pub mod utils;

#[cfg(feature = "sys")]
pub mod sys;

pub mod encoding;

pub use self::error::{Error, Result};
pub use aead::Payload;
pub use protected::Protected;
pub use zeroize::Zeroize;
