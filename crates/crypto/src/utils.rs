use crate::{Error, Result};
use zeroize::Zeroize;

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

#[cfg(test)]
mod tests {
	use crate::{ct::ConstantTimeEqNull, primitives::SALT_LEN, utils::ToArray};

	#[test]
	fn vec_to_array() {
		let vec = vec![1u8; SALT_LEN];
		let array: [u8; SALT_LEN] = vec.clone().to_array().unwrap();

		assert!(!bool::from(vec.ct_eq_null()));
		assert_eq!(vec, array);
		assert_eq!(vec.len(), SALT_LEN);
		assert_eq!(array.len(), SALT_LEN);
	}

	#[test]
	fn slice_to_array() {
		let slice: &[u8] = [1u8; SALT_LEN].as_ref();
		let array: [u8; SALT_LEN] = slice.to_array().unwrap();

		assert!(!bool::from(slice.ct_eq_null()));
		assert_eq!(slice, array);
		assert_eq!(slice.len(), SALT_LEN);
		assert_eq!(array.len(), SALT_LEN);
	}
}
