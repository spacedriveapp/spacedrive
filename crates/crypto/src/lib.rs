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
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(unsafe_code, deprecated_in_future)]
#![allow(
	clippy::missing_errors_doc,
	clippy::module_name_repetitions,
	clippy::similar_names
)]

pub mod crypto;
pub mod ct;
pub mod encoding;
pub mod encrypted;
pub mod error;
pub mod hashing;
pub mod primitives;
pub mod protected;
pub mod rng;
pub mod types;
pub mod utils;
pub mod vault;

#[cfg(all(
	feature = "keyring",
	any(target_os = "macos", target_os = "ios", target_os = "linux")
))]
pub mod keyring;

#[cfg(feature = "sys")]
pub mod sys;

pub use self::error::{Error, Result};
pub use aead::Payload;
pub use protected::Protected;
pub use zeroize::Zeroize;
