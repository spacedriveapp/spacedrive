pub mod file;

#[cfg(feature = "bincode")]
mod bincode;

#[cfg(feature = "bincode")]
pub use self::bincode::{decode, decode_from_reader, encode};

pub use file::header::Header;
