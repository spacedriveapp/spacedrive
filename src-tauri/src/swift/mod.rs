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

extern "C" {
  #[link_name = "get_file_thumbnail_base64"]
  fn get_file_thumbnail_base64_(path: SRString) -> SRString;

  #[link_name = "get_mounts"]
  fn get_mounts_() -> SRObjectArray<Mount>;
}

pub fn get_file_thumbnail_base64(path: &str) -> SRString {
  unsafe { get_file_thumbnail_base64_(path.into()) }
}

pub fn get_mounts() -> SRObjectArray<Mount> {
  unsafe { get_mounts_() }
}
