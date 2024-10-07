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

// pub mod crypto;
pub mod cloud;
pub mod ct;
pub mod erase;
pub mod error;
pub mod primitives;
pub mod protected;
pub mod rng;

pub use error::Error;
pub use protected::Protected;
pub use rng::CryptoRng;

pub use rand_core::{RngCore, SeedableRng};
