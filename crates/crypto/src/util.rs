use rand::{RngCore, SeedableRng};
use subtle::ConstantTimeEq;
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

/// Ideally this should be used for small amounts only.
///
/// It is stack allocated, so be wary.
#[must_use]
pub fn generate_fixed<const I: usize>() -> [u8; I] {
	let mut bytes = [0u8; I];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut bytes);
	bytes
}

/// Ideally this should be used for small amounts only.
#[must_use]
pub fn generate_vec(size: usize) -> Vec<u8> {
	let mut bytes = vec![0u8; size];
	rand_chacha::ChaCha20Rng::from_entropy().fill_bytes(&mut bytes);
	bytes
}

pub fn ensure_not_null(b: &[u8]) -> Result<()> {
	// constant time "not equal" would make this cleaner
	// needs subtle 2.5.0+ though
	(!b.iter().all(|x| x.ct_eq(&0u8).into()))
		.then_some(())
		.ok_or(Error::NullType)
}

pub fn ensure_length(expected: usize, b: &[u8]) -> Result<()> {
	bool::from(b.len().ct_eq(&expected))
		.then_some(())
		.ok_or(Error::LengthMismatch)
}
