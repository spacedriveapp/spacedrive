use std::fmt::Display;
/// The size of 1MiB in bytes
const MIB: u64 = 1_048_576;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const GENERIC_MAXIMUM_FILE_SIZE: u64 = MIB * 64;

/// These are roughly all extensions supported by the `image` crate, as of `v0.24.7`.
///
/// We only support images that have both good encoding and decoding support.
pub const GENERIC_EXTENSIONS: [&str; 16] = [
	"bmp", "dib", "ff", "gif", "ico", "jpg", "jpeg", "png", "pnm", "qoi", "tga", "icb", "vda",
	"vst", "tiff", "tif",
];

#[cfg(feature = "heif")]
pub const HEIF_EXTENSIONS: [&str; 7] = ["heif", "heifs", "heic", "heics", "avif", "avci", "avcs"];

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
#[cfg(feature = "heif")]
pub const HEIF_MAXIMUM_FILE_SIZE: u64 = MIB * 32;

pub const SVG_EXTENSIONS: [&str; 2] = ["svg", "svgz"];

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const SVG_MAXIMUM_FILE_SIZE: u64 = MIB * 24;

/// This is the target pixel count for all SVG images to be rendered at.
///
/// It is 512x512, but if the SVG has a non-1:1 aspect ratio we need to account for that.
pub const SVG_TAGRET_PX: f32 = 262_144_f32;

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "bincode", derive(bincode::Encode, bincode::Decode))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[derive(Debug)]
pub enum ConvertableExtensions {
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
	Heif,
	Heifs,
	Heic,
	Heics,
	Avif,
	Avci,
	Avcs,
	Svg,
	Svgz,
}

impl Display for ConvertableExtensions {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str(&format!("{self:?}"))
	}
}

impl TryFrom<String> for ConvertableExtensions {
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
			"heif" => Ok(Self::Heif),
			"heifs" => Ok(Self::Heifs),
			"heic" => Ok(Self::Heic),
			"heics" => Ok(Self::Heics),
			"avif" => Ok(Self::Avif),
			"avci" => Ok(Self::Avci),
			"avcs" => Ok(Self::Avcs),
			"svg" => Ok(Self::Svg),
			"svgz" => Ok(Self::Svgz),
			_ => Err(crate::Error::Unsupported),
		}
	}
}

#[cfg(feature = "serde")]
impl serde::Serialize for ConvertableExtensions {
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
impl<'de> serde::de::Visitor<'de> for ExtensionVisitor {
	type Value = ConvertableExtensions;

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
impl<'de> serde::Deserialize<'de> for ConvertableExtensions {
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
		.map(String::from)
		.collect();

	#[cfg(not(feature = "heif"))]
	let res = GENERIC_EXTENSIONS
		.into_iter()
		.chain(SVG_EXTENSIONS)
		.map(String::from)
		.collect();

	res
}
