/// The size of 1MiB in bytes
const MIB: u64 = 1_048_576;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const GENERIC_MAXIMUM_FILE_SIZE: u64 = MIB * 64;

/// These are roughly all extensions supported by the `image` crate, as of `v0.24.7`.
///
/// We only support images that have both good encoding and decoding support.
///
// TODO(brxken128): test out ".cur" files, they're an extension of ICO
pub const GENERIC_EXTENSIONS: [&str; 17] = [
	"bmp", "dib", "ff", "gif", "ico", "cur", "jpg", "jpeg", "png", "pnm", "qoi", "tga", "icb",
	"vda", "vst", "tiff", "tif",
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
