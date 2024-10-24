use std::{ffi::OsStr, fmt::Display, path::Path};

/// The size of 1MiB in bytes
const MIB: u64 = 1_048_576;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const MAXIMUM_FILE_SIZE: u64 = MIB * 1024;

/// These are roughly all extensions supported by the `image` crate, as of `v0.24.7`.
///
/// We only support images that have both good encoding and decoding support, without external C-based dependencies (e.g. `avif`)
pub const GENERIC_EXTENSIONS: [&str; 17] = [
	"bmp", "dib", "ff", "gif", "ico", "jpg", "jpeg", "png", "pnm", "qoi", "tga", "icb", "vda",
	"vst", "tiff", "tif", "webp",
];
pub const SVG_EXTENSIONS: [&str; 2] = ["svg", "svgz"];
pub const PDF_EXTENSIONS: [&str; 1] = ["pdf"];
#[cfg(feature = "heif")]
pub const HEIF_EXTENSIONS: [&str; 8] = [
	"hif", "heif", "heifs", "heic", "heics", "avif", "avci", "avcs",
];

// Will be needed for validating HEIF images
// #[cfg(feature = "heif")]
// pub const HEIF_BPS: u8 = 8;

/// The maximum file size that an image can be in order to have a thumbnail generated.
/// This is the target pixel count for all SVG images to be rendered at.
///
/// It is 512x512, but if the SVG has a non-1:1 aspect ratio we need to account for that.
pub const SVG_TARGET_PX: f32 = 262_144f32;

/// The size that PDF pages are rendered at.
///
/// This is 96DPI at standard A4 printer paper size - the target aspect
/// ratio and height are maintained.
pub const PDF_PORTRAIT_RENDER_WIDTH: pdfium_render::prelude::Pixels = 794;
pub const PDF_LANDSCAPE_RENDER_WIDTH: pdfium_render::prelude::Pixels = 1123;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[derive(Debug, Clone, Copy)]
pub enum ConvertibleExtension {
	Bmp,
	Dib,
	Ff,
	Gif,
	Ico,
	Jpg,
	Jpeg,
	Png,
	Pnm,
	Qoi,
	Tga,
	Icb,
	Vda,
	Vst,
	Tiff,
	Tif,
	Hif,
	Heif,
	Heifs,
	Heic,
	Heics,
	Avif,
	Avci,
	Avcs,
	Svg,
	Svgz,
	Pdf,
	Webp,
}

impl ConvertibleExtension {
	#[must_use]
	pub const fn should_rotate(self) -> bool {
		!matches!(
			self,
			Self::Hif
				| Self::Heif | Self::Heifs
				| Self::Heic | Self::Heics
				| Self::Avif | Self::Avci
				| Self::Avcs
		)
	}
}

impl Display for ConvertibleExtension {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{self:?}")
	}
}

impl TryFrom<String> for ConvertibleExtension {
	type Error = crate::Error;

	fn try_from(value: String) -> Result<Self, Self::Error> {
		let v = value.to_lowercase();
		match v.as_str() {
			"bmp" => Ok(Self::Bmp),
			"dib" => Ok(Self::Dib),
			"ff" => Ok(Self::Ff),
			"gif" => Ok(Self::Gif),
			"ico" => Ok(Self::Ico),
			"jpg" => Ok(Self::Jpg),
			"jpeg" => Ok(Self::Jpeg),
			"png" => Ok(Self::Png),
			"pnm" => Ok(Self::Pnm),
			"qoi" => Ok(Self::Qoi),
			"tga" => Ok(Self::Tga),
			"icb" => Ok(Self::Icb),
			"vda" => Ok(Self::Vda),
			"vst" => Ok(Self::Vst),
			"tiff" => Ok(Self::Tiff),
			"tif" => Ok(Self::Tif),
			"hif" => Ok(Self::Hif),
			"heif" => Ok(Self::Heif),
			"heifs" => Ok(Self::Heifs),
			"heic" => Ok(Self::Heic),
			"heics" => Ok(Self::Heics),
			"avif" => Ok(Self::Avif),
			"avci" => Ok(Self::Avci),
			"avcs" => Ok(Self::Avcs),
			"svg" => Ok(Self::Svg),
			"svgz" => Ok(Self::Svgz),
			"pdf" => Ok(Self::Pdf),
			"webp" => Ok(Self::Webp),
			_ => Err(crate::Error::Unsupported),
		}
	}
}

impl TryFrom<&Path> for ConvertibleExtension {
	type Error = crate::Error;

	fn try_from(value: &Path) -> Result<Self, Self::Error> {
		value
			.extension()
			.and_then(OsStr::to_str)
			.map(str::to_string)
			.map_or_else(|| Err(crate::Error::Unsupported), Self::try_from)
	}
}

#[cfg(feature = "serde")]
impl serde::Serialize for ConvertibleExtension {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		serializer.serialize_str(&self.to_string())
	}
}

#[cfg(feature = "serde")]
struct ExtensionVisitor;

#[cfg(feature = "serde")]
impl serde::de::Visitor<'_> for ExtensionVisitor {
	type Value = ConvertibleExtension;

	fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		formatter.write_str("A valid extension string`")
	}

	fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
	where
		E: serde::de::Error,
	{
		Self::Value::try_from(v.to_string()).map_err(|e| E::custom(format!("unknown variant: {e}")))
	}
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ConvertibleExtension {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		deserializer.deserialize_str(ExtensionVisitor)
	}
}

#[inline]
#[must_use]
pub fn all_compatible_extensions() -> Vec<String> {
	#[cfg(feature = "heif")]
	let res = GENERIC_EXTENSIONS
		.into_iter()
		.chain(HEIF_EXTENSIONS)
		.chain(SVG_EXTENSIONS)
		.chain(PDF_EXTENSIONS)
		.map(String::from)
		.collect();

	#[cfg(not(feature = "heif"))]
	let res = GENERIC_EXTENSIONS
		.into_iter()
		.chain(SVG_EXTENSIONS)
		.chain(PDF_EXTENSIONS)
		.map(String::from)
		.collect();

	res
}
