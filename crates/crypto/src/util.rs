use cmov::Cmov;
use rand::{RngCore, SeedableRng};
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

pub(crate) trait ConstantTime {
	fn ct_eq(&self, rhs: &Self) -> bool;
	fn ct_ne(&self, rhs: &Self) -> bool {
		!self.ct_eq(rhs)
	}
}

impl ConstantTime for usize {
	fn ct_eq(&self, rhs: &Self) -> bool {
		let mut x = 0u8;
		x.cmovnz(1u8, u8::from(self == rhs));
		x != 0u8
	}
}

impl ConstantTime for u8 {
	fn ct_eq(&self, rhs: &Self) -> bool {
		let mut x = 0u8;
		x.cmovnz(1u8, Self::from(self == rhs));
		x != 0u8
	}
}

impl<T: ConstantTime> ConstantTime for [T] {
	fn ct_eq(&self, rhs: &Self) -> bool {
		// short-circuit if lengths don't match
		if self.len().ct_ne(&rhs.len()) {
			return false;
		}

		let mut x = 1u8;

		self.iter()
			.zip(rhs.iter())
			.for_each(|(a, b)| x.cmovz(0u8, u8::from(a.ct_eq(b))));

		x != 0u8
	}
}

pub(crate) trait ConstantTimeNull {
	/// Check if the provided value is equivalent to null, in constant time.
	fn ct_eq_null(&self) -> bool;
	/// Check if the provided value is not equivalent to null, in constant time.
	fn ct_ne_null(&self) -> bool {
		!self.ct_eq_null()
	}
}

impl<T: ConstantTime + Default> ConstantTimeNull for [T] {
	fn ct_eq_null(&self) -> bool {
		let mut x = 1u8;
		let d = T::default();

		self.iter()
			.for_each(|i| x.cmovz(0u8, u8::from(i.ct_eq(&d))));
		x != 0u8
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use crate::{
		primitives::{BLOCK_LEN, SALT_LEN},
		util::{ConstantTime, ConstantTimeNull, ToArray},
	};

	// TODO(brxken128): tests for every possible CT case, every impl, etc
	// can probably just test the `ct_eq` implementations, as `ct_ne` is just inverted

	const USIZE1: usize = 64;
	const USIZE2: usize = 56;

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
	fn constant_time_eq_null() {
		assert!(&[0u8; SALT_LEN].ct_eq_null());
	}

	#[test]
	#[should_panic]
	fn constant_time_eq_null_fail() {
		assert!(&[1u8; SALT_LEN].ct_eq_null());
	}

	#[test]
	fn constant_time_ne_null() {
		assert!(&[1u8; SALT_LEN].ct_ne_null());
	}

	#[test]
	#[should_panic]
	fn constant_time_ne_null_fail() {
		assert!(&[0u8; SALT_LEN].ct_ne_null());
	}

	#[test]
	fn constant_time_eq_usize() {
		assert!(USIZE1.ct_eq(&USIZE1));
	}

	#[test]
	#[should_panic]
	fn constant_time_eq_usize_fail() {
		assert!(USIZE1.ct_eq(&USIZE2));
	}

	#[test]
	fn constant_time_ne_usize() {
		assert!(USIZE1.ct_ne(&USIZE2));
	}

	#[test]
	#[should_panic]
	fn constant_time_ne_usize_fail() {
		assert!(USIZE1.ct_ne(&USIZE1));
	}

	#[test]
	fn constant_time_eq() {
		assert!(&[0u8; SALT_LEN].ct_eq(&[0u8; SALT_LEN]));
	}

	#[test]
	fn constant_time_eq_large() {
		assert!(vec![0u8; BLOCK_LEN * 5].ct_eq(&vec![0u8; BLOCK_LEN * 5]));
	}

	#[test]
	#[should_panic]
	fn constant_time_eq_large_fail() {
		assert!(vec![0u8; BLOCK_LEN * 5].ct_eq(&vec![1u8; BLOCK_LEN * 5]));
	}

	#[test]
	#[should_panic]
	fn constant_time_eq_different_bytes() {
		assert!(&[0u8; SALT_LEN].ct_eq(&[1u8; SALT_LEN]));
	}

	#[test]
	#[should_panic]
	fn constant_time_eq_different_length() {
		assert!([0u8; SALT_LEN].as_ref().ct_eq([0u8; 1].as_ref()));
	}

	#[test]
	fn constant_time_ne() {
		assert!(&[0u8; SALT_LEN].ct_ne(&[1u8; SALT_LEN]));
	}

	#[test]
	#[should_panic]
	fn constant_time_ne_fail() {
		assert!([0u8; SALT_LEN].ct_ne(&[0u8; SALT_LEN]));
	}
}
