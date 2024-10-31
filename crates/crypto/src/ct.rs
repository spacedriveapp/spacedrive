use aead::array::{Array, ArraySize};
use cmov::{Cmov, CmovEq};

// The basic principle of most `ct_eq()` functions is to "null" out `x` (which is = 1 by default)
// if any of the values aren't equal. If `x` is nulled out (= 0) at any point, then this will return false,
// otherwise it will remain true and both values will be considered equal.
// This functionality is repeated across all `.iter().zip(...)` methods.

// For some other implementations, we convert the type to bytes before calling `.ct_eq()`,
// making use of the already-made functions.

pub trait ConstantTimeEq {
	fn ct_eq(&self, rhs: &Self) -> Choice;

	fn ct_ne(&self, rhs: &Self) -> Choice {
		!self.ct_eq(rhs)
	}
}

macro_rules! impl_ct_int {
	($($int_type:ident),*) => {
		$(
			impl ConstantTimeEq for $int_type {
				fn ct_eq(&self, rhs: &Self) -> Choice {
					self.to_le_bytes().ct_eq(&rhs.to_le_bytes())
				}
			}
		)*
	};
}

impl_ct_int!(usize, u8, u16, u32, u64, u128);
impl_ct_int!(isize, i8, i16, i32, i64, i128);

impl<T, const I: usize> ConstantTimeEq for [T; I]
where
	T: CmovEq,
{
	fn ct_eq(&self, rhs: &Self) -> Choice {
		let mut x = 1u8;

		self.iter()
			.zip(rhs.iter())
			.for_each(|(l, r)| l.cmovne(r, 0u8, &mut x));

		Choice::from(x)
	}
}

impl<T, N: ArraySize> ConstantTimeEq for Array<T, N>
where
	T: CmovEq,
{
	fn ct_eq(&self, rhs: &Self) -> Choice {
		let mut x = 1u8;

		self.iter()
			.zip(rhs.iter())
			.for_each(|(l, r)| l.cmovne(r, 0u8, &mut x));

		Choice::from(x)
	}
}

impl<T: CmovEq> ConstantTimeEq for [T] {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		// Here we can short-circuit as it's obvious that they're not equal
		if self.len() != rhs.len() {
			return Choice::from(0);
		}

		let mut x = 1u8;

		self.iter()
			.zip(rhs.iter())
			.for_each(|(l, r)| l.cmovne(r, 0u8, &mut x));

		Choice::from(x)
	}
}

impl ConstantTimeEq for String {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		// Here we are just able to convert both values to bytes and use the
		// appropriate methods to compare the two in constant-time.
		self.as_bytes().ct_eq(rhs.as_bytes())
	}
}

impl ConstantTimeEq for &str {
	fn ct_eq(&self, rhs: &Self) -> Choice {
		// Here we are just able to convert both values to bytes and use the
		// appropriate methods to compare the two in constant-time.
		self.as_bytes().ct_eq(rhs.as_bytes())
	}
}

#[derive(Copy, Clone, Debug)]
pub struct Choice(u8);

impl Choice {
	#[inline]
	#[must_use]
	pub fn unwrap_u8(&self) -> u8 {
		// could use an unsafe volatile read as an optimization barrier
		// i think cmov does a great job at being the barrier as well though
		let mut x = 0u8;
		x.cmovnz(&1, self.0);
		x
	}
}

impl std::ops::Not for Choice {
	type Output = Self;

	#[inline]
	fn not(self) -> Self {
		let mut x = 0u8;
		x.cmovz(&1, self.0);
		Self::from(x)
	}
}

impl std::ops::BitAnd for Choice {
	type Output = Self;

	#[inline]
	fn bitand(self, rhs: Self) -> Self::Output {
		let mut x = 0u8;
		x.cmovnz(&1u8, self.0 & rhs.0);
		Self::from(x)
	}
}

impl std::ops::BitAndAssign for Choice {
	#[inline]
	fn bitand_assign(&mut self, rhs: Self) {
		let mut x = 0u8;
		x.cmovnz(&1u8, self.0 & rhs.0);
		*self = Self::from(x);
	}
}

impl From<u8> for Choice {
	#[inline]
	fn from(input: u8) -> Self {
		let mut x = 0u8;
		x.cmovnz(&1, input);
		Self(x)
	}
}

impl From<Choice> for bool {
	/// Convert the `Choice` wrapper into a `bool`, depending on whether
	/// the underlying `u8` is equal to `0` or not.
	#[inline]
	fn from(input: Choice) -> Self {
		let mut x = 0u8;
		x.cmovnz(&1, input.unwrap_u8());
		x != 0
	}
}

pub trait ConstantTimeEqNull {
	/// Check if the provided value is equivalent to null, in constant time.
	fn ct_eq_null(&self) -> Choice;
	/// Check if the provided value is not equivalent to null, in constant time.
	#[inline]
	fn ct_ne_null(&self) -> Choice {
		!self.ct_eq_null()
	}
}

impl ConstantTimeEqNull for [u8] {
	#[inline]
	fn ct_eq_null(&self) -> Choice {
		let mut x = 1u8;
		self.iter().for_each(|b| b.cmovne(&0, 0u8, &mut x));
		Choice::from(x)
	}
}

#[cfg(test)]
mod tests {

	use aead::array::Array;
	use typenum::consts::U32;

	use crate::ct::{ConstantTimeEq, ConstantTimeEqNull};

	#[test]
	fn eq_null() {
		assert!(bool::from([0u8; 16].ct_eq_null()));
	}

	#[test]
	#[should_panic(expected = "assertion")]
	fn eq_null_fail() {
		assert!(bool::from([1u8; 16].ct_eq_null()));
	}

	#[test]
	fn ne_null() {
		assert!(bool::from([1u8; 16].ct_ne_null()));
	}

	#[test]
	#[should_panic(expected = "assertion")]
	fn ne_null_fail() {
		assert!(bool::from([0u8; 16].ct_ne_null()));
	}

	#[test]
	fn generic_array_eq() {
		assert!(bool::from(Array::<u8, U32>::ct_eq(
			&Array::from([0u8; 32]),
			&Array::from([0u8; 32]),
		)));
	}
	#[test]
	#[should_panic(expected = "assertion")]
	fn generic_array_eq_fail() {
		assert!(bool::from(Array::<u8, U32>::ct_eq(
			&Array::from([0u8; 32]),
			&Array::from([1u8; 32]),
		)));
	}

	#[test]
	fn generic_array_ne() {
		assert!(bool::from(Array::<u8, U32>::ct_ne(
			&Array::from([0u8; 32]),
			&Array::from([1u8; 32]),
		)));
	}

	#[test]
	#[should_panic(expected = "assertion")]
	fn generic_array_ne_fail() {
		assert!(bool::from(Array::<u8, U32>::ct_ne(
			&Array::from([0u8; 32]),
			&Array::from([0u8; 32]),
		)));
	}

	macro_rules! create_tests {
		(($sample1:expr, $sample2:expr), $($item_type:ty),*) => {
			$(
				paste::paste! {
					#[test]
					fn [<eq_ $item_type:lower>]() {
						let x: $item_type = $sample1;
						assert!(bool::from(x.ct_eq(&$sample1)));
					}

					#[test]
					#[should_panic(expected = "assertion")]
					fn [<eq_ $item_type:lower _fail>]() {
						let x: $item_type = $sample1;
						assert!(bool::from(x.ct_eq(&$sample2)));
					}

					#[test]
					fn [<ne_ $item_type:lower>]() {
						let x: $item_type = $sample1;
						assert!(bool::from(x.ct_ne(&$sample2)));
					}

					#[test]
					#[should_panic(expected = "assertion")]
					fn [<ne_ $item_type:lower _fail>]() {
						let x: $item_type = $sample1;
						assert!(bool::from(x.ct_ne(&$sample1)));
					}
				}
			)*

		};
	}

	create_tests!((0, 1), usize, u8, u16, u32, u64, u128);
	create_tests!((0, 1), isize, i8, i16, i32, i64, i128);

	create_tests!((String::from("test"), String::from("Test")), String);
}
