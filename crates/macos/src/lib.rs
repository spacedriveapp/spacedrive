#![cfg(target_os = "macos")]

use swift_rs::*;

#[repr(C)]
pub struct Volume {
	name: SRString,
	path: SRString,
	total_capacity: Int,
	available_capacity: Int,
	is_removable: Bool,
	is_ejectable: Bool,
	is_root_filesystem: Bool,
}

swift!(pub fn get_file_thumbnail_base64(name: &SRString) -> SRString);
swift!(pub fn get_mounts() -> SRObjectArray<Volume>);
