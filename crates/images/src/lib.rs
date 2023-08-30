mod consts;
mod error;
pub mod formatter;
mod raw;

#[cfg(not(target_os = "linux"))]
mod heif;
#[cfg(not(target_os = "linux"))]
pub use consts::HEIF_EXTENSIONS;
#[cfg(not(target_os = "linux"))]
pub use heif::heif_to_dynamic_image;

pub use consts::RAW_EXTENSIONS;
pub use error::{Error, Result};
