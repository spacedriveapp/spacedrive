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

pub(crate) mod consts;
mod dimensions;
mod error;
mod flash;
mod image;
mod location;
mod orientation;
mod profile;
mod time;
pub(crate) mod utils;

pub use consts::DMS_DIVISION;
pub use dimensions::Dimensions;
pub use error::{Error, Result};
pub use flash::{Flash, FlashMode, FlashValue};
pub use image::{CameraData, ExifReader, MediaDataImage};
pub use location::MediaLocation;
pub use orientation::Orientation;
pub use profile::ColorProfile;
pub use time::MediaTime;
