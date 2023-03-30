use rand_chacha::{
	rand_core::{RngCore, SeedableRng},
	ChaCha20Rng,
};
use zeroize::Zeroize;

use crate::{Error, Result};

pub(crate) trait ToArray {
	fn to_array<const I: usize>(self) -> Result<[u8; I]>;
}

impl ToArray for Vec<u8> {
	/// This function uses `try_into()`, and calls `zeroize` in the event of an error.
	fn to_array<const I: usize>(self) -> Result<[u8; I]> {
		self.try_into().map_err(|mut b: Self| {
			b.zeroize();
			Error::LengthMismatch
		})
	}
}

impl ToArray for &[u8] {
	/// **Using this can be risky - ensure that you `zeroize` the source buffer before returning.**
	///
	/// `zeroize` cannot be called on the input as we do not have ownership.
	///
	/// This function copies `self` into a `Vec`, before using the `ToArray` implementation for `Vec<u8>`
	fn to_array<const I: usize>(self) -> Result<[u8; I]> {
		self.to_vec().to_array()
	}
}

/// Used to generate completely random bytes, with the use of `ChaCha20`
///
/// Ideally this should be used for small amounts only (as it's stack allocated)
#[must_use]
pub fn generate_fixed<const I: usize>() -> [u8; I] {
	let mut bytes = [0u8; I];
	ChaCha20Rng::from_entropy().fill_bytes(&mut bytes);
	bytes
}

/// Used to generate completely random bytes, with the use of `ChaCha20`
#[must_use]
pub fn generate_vec(size: usize) -> Vec<u8> {
	let mut bytes = vec![0u8; size];
	ChaCha20Rng::from_entropy().fill_bytes(&mut bytes);
	bytes
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use crate::{primitives::SALT_LEN, utils::ToArray};

	#[test]
	fn vec_to_array() {
		let vec = vec![1u8; SALT_LEN];
		let array: [u8; SALT_LEN] = vec.clone().to_array().unwrap();

		assert_eq!(vec, array);
		assert_eq!(vec.len(), SALT_LEN);
		assert_eq!(array.len(), SALT_LEN);
	}

	#[test]
	fn slice_to_array() {
		let slice = [1u8; SALT_LEN].as_ref();
		let array: [u8; SALT_LEN] = slice.to_array().unwrap();

		assert_eq!(slice, array);
		assert_eq!(slice.len(), SALT_LEN);
		assert_eq!(array.len(), SALT_LEN);
	}

	#[test]
	fn generate_bytes() {
		let bytes = super::generate_vec(SALT_LEN);
		let bytes2 = super::generate_vec(SALT_LEN);

		assert_ne!(bytes, bytes2);
		assert_eq!(bytes.len(), SALT_LEN);
		assert_eq!(bytes2.len(), SALT_LEN);
	}

	#[test]
	fn generate_fixed() {
		let bytes: [u8; SALT_LEN] = super::generate_fixed();
		let bytes2: [u8; SALT_LEN] = super::generate_fixed();

		assert_ne!(bytes, bytes2);
		assert_eq!(bytes.len(), SALT_LEN);
		assert_eq!(bytes2.len(), SALT_LEN);
	}
}
