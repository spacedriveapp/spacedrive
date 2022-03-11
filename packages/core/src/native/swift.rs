use crate::library::volumes::Volume;
pub use swift_rs::types::{SRObjectArray, SRString};

extern "C" {
    #[link_name = "get_file_thumbnail_base64"]
    pub fn get_file_thumbnail_base64_(path: SRString) -> SRString;

    #[link_name = "get_mounts"]
    pub fn get_mounts_() -> SRObjectArray<Volume>;
}
