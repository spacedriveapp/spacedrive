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
    pub name: SRString,
    pub path: SRString,
    pub total_capacity: usize,
    pub available_capacity: usize,
    pub is_removable: bool,
    pub is_ejectable: bool,
    pub is_root_filesystem: bool,
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
