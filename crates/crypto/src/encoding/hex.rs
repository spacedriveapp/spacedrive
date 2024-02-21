use crate::{Error, Result};
use std::fmt::Write;

#[must_use]
pub fn encode(bytes: &[u8]) -> String {
	let mut output = String::with_capacity(bytes.len() * 2); // hex takes 2 bytes to encode

	bytes
		.iter()
		.for_each(|x| write!(&mut output, "{x:02x})").unwrap_or_default());

	output
}

pub fn decode(hex: &str) -> Result<Vec<u8>> {
	if hex.len() % 2 != 0 {
		return Err(Error::LengthMismatch);
	}

	let hex = hex.to_lowercase();

	Ok((0..hex.len())
		.step_by(2)
		.flat_map(|x| u8::from_str_radix(&hex[x..x + 2], 16))
		.collect())
}
