use crate::{Error, Result};

pub fn decode<T>(bytes: &[u8]) -> Result<T>
where
	T: bincode::Decode,
{
	bincode::decode_from_slice::<T, bincode::config::Configuration>(
		bytes,
		bincode::config::standard(),
	)
	.map(|t| t.0)
	.map_err(Error::BincodeDecode)
}

pub fn encode<T>(object: T) -> Result<Vec<u8>>
where
	T: bincode::Encode,
{
	bincode::encode_to_vec(object, bincode::config::standard()).map_err(Error::BincodeEncode)
}
