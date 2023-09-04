use once_cell::sync::Lazy;
use std::ffi::OsString;

/// The size of 1MiB in bytes
const MIB: u64 = 1_048_576;

#[cfg(all(feature = "heif", not(target_os = "linux")))]
pub static HEIF_EXTENSIONS: Lazy<Vec<OsString>> = Lazy::new(|| {
	["heif", "heifs", "heic", "heics", "avif", "avci", "avcs"]
		.iter()
		.map(OsString::from)
		.collect()
});

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
#[cfg(all(feature = "heif", not(target_os = "linux")))]
pub const HEIF_MAXIMUM_FILE_SIZE: u64 = MIB * 24;

pub static SVG_EXTENSIONS: Lazy<Vec<OsString>> =
	Lazy::new(|| ["svg", "svgz"].iter().map(OsString::from).collect());

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const SVG_MAXIMUM_FILE_SIZE: u64 = MIB * 24;

/// This is not all `RAW` extensions, but a subset of the most common ones,
/// and the ones that the `rawloader` crate are most likely to support.
pub static RAW_EXTENSIONS: Lazy<Vec<OsString>> = Lazy::new(|| {
	[
		"arw", "crw", "cr2", "cr3", "dng", "mdc", "mrw", "orf", "r3d", "sr2", "srf", "srw", "raw",
	]
	.iter()
	.map(OsString::from)
	.collect()
});

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const RAW_MAXIMUM_FILE_SIZE: u64 = MIB * 48;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub const GENERIC_MAXIMUM_FILE_SIZE: u64 = MIB * 24;

// This is the *full* list of RAW extensions, I'm not sure which we're 100% going to
// be able to support so I chose the most common ones
// pub const RAW_EXTENSIONS: [&str; 43] = [
// 	"3fr", "ari", "arw", "bay", "braw", "crw", "cr2", "cr3", "cap", "data", "dcs", "dcr", "dng",
// 	"drf", "eip", "erf", "fff", "gpr", "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef",
// 	"nrw", "obm", "orf", "pef", "ptx", "pxn", "r3d", "raf", "raw", "rwl", "rw2", "rwz", "sr2",
// 	"srf", "srw", "tif", "x3f",
// ];
