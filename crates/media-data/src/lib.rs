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

mod composite;
pub(crate) mod consts;
mod dimensions;
mod error;
mod flash;
mod image;
mod location;
mod orientation;
mod profile;
mod time;

pub use composite::Composite;
pub use consts::DMS_DIVISION;
pub use dimensions::Dimensions;
pub use error::{Error, Result};
pub use flash::{Flash, FlashMode, FlashValue};
pub use image::{ExifReader, ImageData, MediaDataImage};
pub use location::MediaLocation;
pub use orientation::Orientation;
pub use profile::ColorProfile;
pub use time::MediaTime;

pub trait MediaDataCore {
	type Data;

	fn from_path(path: impl AsRef<std::path::Path>) -> Option<Self::Data>;
	fn from_slice(bytes: &[u8]) -> Option<Self::Data>;
}

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum MediaData {
	Image(Box<MediaDataImage>),
	Video(u8),
}

impl MediaData {
	#[must_use]
	pub fn inner(&self) -> &impl MediaDataCore {
		match self {
			Self::Image(x) => &**x,
			Self::Video(_) => unreachable!(),
		}
	}
}

impl MediaDataCore for MediaDataImage {
	type Data = Self;

	fn from_path(path: impl AsRef<std::path::Path>) -> Option<Self::Data> {
		Self::from_path(path).ok()
	}

	fn from_slice(bytes: &[u8]) -> Option<Self::Data> {
		Self::from_slice(bytes).ok()
	}
}
