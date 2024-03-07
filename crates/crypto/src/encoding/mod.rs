mod bincode;
pub mod file;

pub use self::bincode::{decode, decode_from_reader, encode};

pub use file::header::Header;
