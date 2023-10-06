/// The size of 1MiB in bytes
const MIB: u64 = 1_048_576;

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

/// The size that SVG images are rendered at.
pub const SVG_RENDER_SIZE: u32 = 512;

pub const PDF_EXTENSION: &str = "pdf";

/// The size that PDF pages are rendered at.
pub const PDF_RENDER_SIZE: i32 = 1024;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const GENERIC_MAXIMUM_FILE_SIZE: u64 = MIB * 64;
