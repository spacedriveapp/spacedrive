// This file must use the following macros to select specific native bindings.
// #[cfg(target_os = "macos")]
// #[cfg(target_os = "linux")]
// #[cfg(target_os = "windows")]
#[cfg(target_os = "macos")]
use super::swift;
use crate::library::volumes::Volume;
use swift_rs::types::{SRObjectArray, SRString};

pub fn get_file_thumbnail_base64(path: &str) -> SRString {
	#[cfg(target_os = "macos")]
	unsafe {
		swift::get_file_thumbnail_base64_(path.into())
	}
}

pub fn get_mounts() -> SRObjectArray<Volume> {
	#[cfg(target_os = "macos")]
	unsafe {
		swift::get_mounts_()
	}
	// #[cfg(target_os = "macos")]

	// println!("getting mounts..");
	// let mut mounts: Vec<Volume> = Vec::new();
	// let swift_mounts = unsafe { swift::get_mounts_() };
	// println!("mounts: {:?}", swift_mounts);

	// for mount in swift_mounts.iter() {
	//     println!("mount: {:?}", *mount);
	//     // mounts.push((&**mount).clone());
	// }
}
