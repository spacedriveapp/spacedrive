use super::ExifReader;
use exif::Tag;
use std::fmt::Display;

#[derive(
	Default, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type,
)]
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
	/// This is used for quickly sourcing a [`ColorProfile`] data from an [`ExifReader`]
	pub fn from_reader(reader: &ExifReader) -> Option<Self> {
		reader.get_tag_int(Tag::CustomRendered).map(Into::into)
	}
}

impl From<u32> for ColorProfile {
	fn from(value: u32) -> Self {
		match value {
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
