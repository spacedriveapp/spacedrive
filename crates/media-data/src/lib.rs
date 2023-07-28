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
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

pub mod consts;
mod dimensions;
mod error;
mod image;
mod location;
mod orientation;
mod time;

pub use consts::DMS_DIVISION;
pub use dimensions::Dimensions;
pub use error::{Error, Result};
pub use image::{CameraData, ExifReader, MediaDataImage};
pub use location::Location;
pub use orientation::Orientation;
pub use time::MediaTime;
