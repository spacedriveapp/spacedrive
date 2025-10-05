pub mod error;

// Re-export commonly used error types
pub use error::{FileIOError, NonUtf8PathError};

// Chain optional iterators utility
pub fn chain_optional_iter<T>(
	required: impl IntoIterator<Item = T>,
	optional: Option<impl IntoIterator<Item = T>>,
) -> impl Iterator<Item = T> {
	required.into_iter().chain(optional.into_iter().flatten())
}

// Frontend compatibility utilities for large integers
// JavaScript can't handle i64/u64 properly, so we convert to tuples

/// Convert i64 to a tuple for frontend compatibility
pub fn i64_to_frontend(value: i64) -> (i32, u32) {
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	{
		// Split into (high, low) parts
		((value >> 32) as i32, value as u32)
	}
}

/// Convert u64 to a tuple for frontend compatibility
pub fn u64_to_frontend(value: u64) -> (u32, u32) {
	#[allow(clippy::cast_possible_truncation)]
	{
		// Split into (high, low) parts
		((value >> 32) as u32, value as u32)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_chain_optional_iter() {
		let required = vec![1, 2, 3];
		let optional = Some(vec![4, 5, 6]);
		let result: Vec<_> = chain_optional_iter(required, optional).collect();
		assert_eq!(result, vec![1, 2, 3, 4, 5, 6]);

		let required = vec![1, 2, 3];
		let optional: Option<Vec<i32>> = None;
		let result: Vec<_> = chain_optional_iter(required, optional).collect();
		assert_eq!(result, vec![1, 2, 3]);
	}

	#[test]
	fn test_i64_to_frontend() {
		let value: i64 = 0x123456789ABCDEF0_u64 as i64;
		let (high, low) = i64_to_frontend(value);
		assert_eq!(high, 0x12345678_i32);
		assert_eq!(low, 0x9ABCDEF0_u32);
	}

	#[test]
	fn test_u64_to_frontend() {
		let value: u64 = 0x123456789ABCDEF0;
		let (high, low) = u64_to_frontend(value);
		assert_eq!(high, 0x12345678_u32);
		assert_eq!(low, 0x9ABCDEF0_u32);
	}
}
