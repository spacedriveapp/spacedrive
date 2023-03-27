use cmov::Cmov;
use subtle::Choice;

pub trait ConstantTimeEqNull {
	/// Check if the provided value is equivalent to null, in constant time.
	fn ct_eq_null(&self) -> Choice;
	/// Check if the provided value is not equivalent to null, in constant time.
	fn ct_ne_null(&self) -> Choice {
		!self.ct_eq_null()
	}
}

impl ConstantTimeEqNull for [u8] {
	fn ct_eq_null(&self) -> Choice {
		let mut x = 1u8;
		self.iter().for_each(|i| x.cmovnz(&0u8, *i));
		Choice::from(x)
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use crate::{ct::ConstantTimeEqNull, primitives::SALT_LEN};

	#[test]
	fn constant_time_ne_null() {
		assert!(bool::from([1u8; SALT_LEN].ct_ne_null()));
	}

	#[test]
	#[should_panic]
	fn constant_time_ne_null_fail() {
		assert!(bool::from([0u8; SALT_LEN].ct_ne_null()));
	}
}
