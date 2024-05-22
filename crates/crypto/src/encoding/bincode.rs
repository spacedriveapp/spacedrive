pub const CONFIG: Configuration = bincode::config::standard();

use crate::{Error, Result};
use bincode::{config::Configuration, de::read::Reader};

pub fn decode<T>(bytes: &[u8]) -> Result<T>
where
	T: bincode::Decode,
{
	bincode::decode_from_slice::<T, Configuration>(bytes, CONFIG)
		.map(|t| t.0)
		.map_err(Error::BincodeDecode)
}

pub fn decode_from_reader<R: Reader, T>(reader: R) -> Result<T>
where
	T: bincode::Decode,
{
	bincode::decode_from_reader::<T, R, Configuration>(reader, CONFIG).map_err(Error::BincodeDecode)
}

pub fn encode<T>(object: &T) -> Result<Vec<u8>>
where
	T: bincode::Encode,
{
	bincode::encode_to_vec(object, CONFIG).map_err(Error::BincodeEncode)
}

// TODO(brxken128): this should probably go but it's convenient
