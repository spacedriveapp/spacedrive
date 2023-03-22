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

#[macro_export]
// `assert_eq!` but constant-time and opaque
macro_rules! assert_ct_eq {
	($left:expr, $right:expr) => {
		match (&$left, &$right) {
			(left_val, right_val) => {
				if !bool::from(left_val.ct_eq(right_val)) {
					panic!("assertion failed")
				}
			}
		}
	};
}

#[macro_export]
// `assert_ne!` but constant-time and opaque
macro_rules! assert_ct_ne {
	($left:expr, $right:expr) => {
		match (&$left, &$right) {
			(left_val, right_val) => {
				if bool::from(left_val.ct_eq(right_val)) {
					panic!("assertion failed")
				}
			}
		}
	};
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use crate::{assert_ct_eq, primitives::SALT_LEN, util::ToArray};
	use subtle::ConstantTimeEq;

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

	#[test]
	fn ensure_not_null() {
		super::ensure_not_null(&[1u8; SALT_LEN]).unwrap();
	}

	#[test]
	#[should_panic(expected = "NullType")]
	fn ensure_not_null_fail() {
		super::ensure_not_null(&[0u8; SALT_LEN]).unwrap();
	}

	#[test]
	fn ensure_length() {
		super::ensure_length(SALT_LEN, &[1u8; SALT_LEN]).unwrap();
	}

	#[test]
	#[should_panic(expected = "LengthMismatch")]
	fn ensure_length_fail() {
		super::ensure_length(SALT_LEN - 1, &[1u8; SALT_LEN]).unwrap();
	}

	#[test]
	fn assert_ct_eq() {
		assert_ct_eq!(1u8, 1u8);
		assert_ct_eq!([23u8; SALT_LEN], [23u8; SALT_LEN]);
	}

	#[test]
	#[should_panic]
	fn assert_ct_eq_fail() {
		assert_ct_eq!(1u8, 2u8);
	}

	#[test]
	fn assert_ct_ne() {
		assert_ct_ne!(1u8, 2u8);
		assert_ct_ne!([23u8; SALT_LEN], [20u8; SALT_LEN]);
	}

	#[test]
	#[should_panic]
	fn assert_ct_ne_fail() {
		assert_ct_ne!(1u8, 1u8);
	}
}
