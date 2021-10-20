use swift_rs::types::SRString;

extern "C" {
  #[link_name = "get_file_thumbnail_base64"]
  fn get_file_thumbnail_base64_(path: &SRString) -> &'static SRString;
}

pub fn get_file_thumbnail_base64(path: &str) -> &'static SRString {
  unsafe { get_file_thumbnail_base64_(path.into()) }
}
