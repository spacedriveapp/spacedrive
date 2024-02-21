#[cfg(feature = "bincode")]
mod bincode;
pub mod file;
pub mod hex;

#[cfg(feature = "bincode")]
pub use self::bincode::{decode, decode_from_reader, encode};

pub use file::header::Header;
