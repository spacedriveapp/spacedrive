// This file must use the following macros to select specific native bindings.
// #[cfg(target_os = "macos")]
// #[cfg(target_os = "linux")]
// #[cfg(target_os = "windows")]
#[cfg(target_os = "macos")]
use super::swift;

use serde::Serialize;

use swift_rs::types::{SRObjectArray, SRString};

#[derive(Serialize)]
#[repr(C)]
pub struct Mount {
    name: SRString,
    path: SRString,
    total_capacity: usize,
    available_capacity: usize,
    is_removable: bool,
    is_ejectable: bool,
    is_root_filesystem: bool,
}

pub fn get_file_thumbnail_base64(path: &str) -> SRString {
    #[cfg(target_os = "macos")]
    unsafe {
        swift::get_file_thumbnail_base64_(path.into())
    }
}

pub fn get_mounts() -> SRObjectArray<Mount> {
    #[cfg(target_os = "macos")]
    unsafe {
        swift::get_mounts_()
    }
}
