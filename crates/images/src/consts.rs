/// The size of 1MiB in bytes
const MIB: u64 = 1_048_576;

#[cfg(all(
	feature = "heif",
	any(not(any(target_os = "linux", target_os = "windows")), heif_images)
))]
pub const HEIF_EXTENSIONS: [&str; 7] = ["heif", "heifs", "heic", "heics", "avif", "avci", "avcs"];

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
#[cfg(all(
	feature = "heif",
	any(not(any(target_os = "linux", target_os = "windows")), heif_images)
))]
pub const HEIF_MAXIMUM_FILE_SIZE: u64 = MIB * 32;

pub const SVG_EXTENSIONS: [&str; 2] = ["svg", "svgz"];

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const SVG_MAXIMUM_FILE_SIZE: u64 = MIB * 24;

/// The size that SVG images are rendered at, assuming they are square.
// TODO(brxken128): check for non-1:1 SVG images and create a function to resize
// them while maintaining the aspect ratio.
pub const SVG_RENDER_SIZE: u32 = 512;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const GENERIC_MAXIMUM_FILE_SIZE: u64 = MIB * 64;
