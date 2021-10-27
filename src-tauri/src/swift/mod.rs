use anyhow::Result;
use serde::{Deserialize, Serialize};
use swift_rs::types::SRString;

extern "C" {
  #[link_name = "get_file_thumbnail_base64"]
  fn get_file_thumbnail_base64_(path: &SRString) -> &'static SRString;
  #[link_name = "get_mounts"]
  fn get_mounts_() -> &'static SRString;
}

pub fn get_file_thumbnail_base64(path: &str) -> &'static SRString {
  unsafe { get_file_thumbnail_base64_(path.into()) }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mount {
  pub name: String,
  pub path: String,
  pub total_capacity: u64,
  pub available_capacity: u64,
  pub is_removable: bool,
  pub is_ejectable: bool,
  pub is_root_filesystem: bool,
}
pub fn get_mounts() -> Result<Vec<Mount>> {
  let func_res = unsafe { get_mounts_() };
  let mounts_json = func_res.to_string();
  let mounts: Vec<Mount> = serde_json::from_str(&mounts_json)?;

  println!("mounts: {:?}", mounts);
  Ok(mounts)
}
