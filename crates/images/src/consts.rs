#[cfg(not(target_os = "linux"))]
pub const HEIF_EXTENSIONS: [&str; 7] = ["heif", "heifs", "heic", "heics", "avif", "avci", "avcs"];

pub const RAW_EXTENSIONS: [&str; 13] = [
	"arw", "crw", "cr2", "cr3", "dng", "mdc", "mrw", "orf", "r3d", "sr2", "srf", "srw", "raw",
];

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub(crate) const HEIF_MAXIMUM_FILE_SIZE: u64 = 1048576 * 24;

/// The maximum file size that an image can be in order to have a thumbnail generated.
///
/// This value is in MiB.
pub(crate) const RAW_MAXIMUM_FILE_SIZE: u64 = 1048576 * 64;

// pub(crate) const FN_MAPS: [[(&[&str], &fn())]; 2] = [
// 	[(&HEIF_EXTENSIONS, heif_to_dynamic_image)],
// 	[(&RAW_EXTENSIONS, raw_to_dynamic_image)],
// ];

// This is the *full* list of RAW extensions, I'm not sure which we're 100% going to
// be able to support so I chose the most common ones
// pub const RAW_EXTENSIONS: [&str; 43] = [
// 	"3fr", "ari", "arw", "bay", "braw", "crw", "cr2", "cr3", "cap", "data", "dcs", "dcr", "dng",
// 	"drf", "eip", "erf", "fff", "gpr", "iiq", "k25", "kdc", "mdc", "mef", "mos", "mrw", "nef",
// 	"nrw", "obm", "orf", "pef", "ptx", "pxn", "r3d", "raf", "raw", "rwl", "rw2", "rwz", "sr2",
// 	"srf", "srw", "tif", "x3f",
// ];
