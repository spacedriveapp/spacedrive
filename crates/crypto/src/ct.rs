use cmov::Cmov;

pub(crate) trait ConstantTimeEq {
	fn ct_eq(&self, rhs: &Self) -> bool;
	fn ct_ne(&self, rhs: &Self) -> bool {
		!self.ct_eq(rhs)
	}
}

impl ConstantTimeEq for usize {
	fn ct_eq(&self, rhs: &Self) -> bool {
		let mut x = 0u8;
		x.cmovnz(1u8, u8::from(self == rhs));
		x != 0u8
	}
}

impl ConstantTimeEq for u8 {
	fn ct_eq(&self, rhs: &Self) -> bool {
		let mut x = 0u8;
		x.cmovnz(1u8, Self::from(self == rhs));
		x != 0u8
	}
}

impl<T: ConstantTimeEq> ConstantTimeEq for [T] {
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

pub(crate) trait ConstantTimeEqNull {
	/// Check if the provided value is equivalent to null, in constant time.
	fn ct_eq_null(&self) -> bool;
	/// Check if the provided value is not equivalent to null, in constant time.
	fn ct_ne_null(&self) -> bool {
		!self.ct_eq_null()
	}
}

impl<T: ConstantTimeEq + Default> ConstantTimeEqNull for [T] {
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
		ct::{ConstantTimeEq, ConstantTimeEqNull},
		primitives::{BLOCK_LEN, SALT_LEN},
	};

	// TODO(brxken128): tests for every possible CT case, every impl, etc
	// can probably just test the `ct_eq` implementations, as `ct_ne` is just inverted

	const USIZE1: usize = 64;
	const USIZE2: usize = 56;

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
