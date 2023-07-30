use crate::ExifReader;
use std::{fmt::Display, path::Path};

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub enum ColorProfile {
	#[default]
	Normal,
	Custom,
	HDRNoOriginal,
	HDRWithOriginal,
	OriginalForHDR,
	Panorama,
	PortraitHDR,
	Portrait,
}

impl ColorProfile {
	/// This is used for quickly sourcing [`ColorProfile`] data from a path, to be later used by one of the modification functions.
	// https://github.com/rust-lang/rust-clippy/issues/11087
	#[allow(clippy::future_not_send)]
	pub async fn source_color_profile<P: AsRef<Path>>(path: P) -> Option<Self> {
		let reader = ExifReader::from_path(path).await.ok()?;
		reader
			.get_color_profile_ints()
			.map(Self::int_to_color_profile)
	}

	/// This follows the EXIF specification as to how images are supposed to be rotated/flipped/etc depending on their associated value
	pub(crate) const fn int_to_color_profile(i: u32) -> Self {
		match i {
			0 => Self::Custom,
			2 => Self::HDRNoOriginal,
			3 => Self::HDRWithOriginal,
			4 => Self::OriginalForHDR,
			6 => Self::Panorama,
			7 => Self::Portrait,
			8 => Self::PortraitHDR,
			_ => Self::Normal,
		}
	}
}

impl Display for ColorProfile {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Normal => f.write_str("Normal"),
			Self::Custom => f.write_str("Custom"),
			Self::HDRNoOriginal => f.write_str("HDR (with no original saved)"),
			Self::HDRWithOriginal => f.write_str("HDR (with original saved)"),
			Self::OriginalForHDR => f.write_str("Original for HDR image"),
			Self::Panorama => f.write_str("Panorama"),
			Self::Portrait => f.write_str("Portrait"),
			Self::PortraitHDR => f.write_str("HDR Portrait"),
		}
	}
}
