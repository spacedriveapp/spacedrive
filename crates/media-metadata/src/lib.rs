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

pub mod audio;
mod error;
pub mod image;
pub mod video;

pub use audio::AudioMetadata;
pub use error::{Error, Result};
pub use image::ImageMetadata;
pub use video::VideoMetadata;

#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "type")]
pub enum MediaMetadata {
	Image(Box<ImageMetadata>),
	Video(Box<VideoMetadata>),
	Audio(Box<AudioMetadata>),
}
