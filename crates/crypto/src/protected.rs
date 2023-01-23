//! This is a basic wrapper for secret/hidden values
//!
//! It's worth noting that this wrapper does not provide additional security that you can't get manually, it just makes it a LOT easier.
//!
//! It implements zeroize-on-drop, meaning the data is securely erased from memory once it goes out of scope.
//! You may call `drop()` prematurely if you wish to erase it sooner.
//!
//! `Protected` values are also hidden from `fmt::Debug`, and will display `[REDACTED]` instead.
//!
//! The only way to access the data within a `Protected` value is to call `.expose()` - this is to prevent accidental leakage.
//! This also makes any `Protected` value easier to audit, as you are able to quickly view wherever the data is accessed.
//!
//! `Protected` values are not able to be copied within memory, to prevent accidental leakage. They are able to be `cloned` however - but this is always explicit and you will be aware of it.
//!
//! I'd like to give a huge thank you to the authors of the [secrecy crate](https://crates.io/crates/secrecy),
//! as that crate's functionality inspired this implementation.
//!
//! # Examples
//!
//! ```rust
//! use sd_crypto::Protected;
//!
//! let secret_data = "this is classified information".to_string();
//! let protected_data = Protected::new(secret_data);
//!
//! // the only way to access the data within the `Protected` wrapper
//! // is by calling `.expose()`
//! let value = protected_data.expose();
//! ```
//!
use std::{fmt::Debug, mem::swap};
use zeroize::Zeroize;

#[derive(Clone)]
pub struct Protected<T>
where
	T: Zeroize,
{
	data: T,
}

impl<T> std::ops::Deref for Protected<T>
where
	T: Zeroize,
{
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.data
	}
}

impl<T> Protected<T>
where
	T: Zeroize,
{
	pub const fn new(value: T) -> Self {
		Self { data: value }
	}

	pub const fn expose(&self) -> &T {
		&self.data
	}

	pub fn zeroize(mut self) {
		self.data.zeroize();
	}
}

impl<T> Protected<T>
where
	T: Zeroize + Default,
{
	pub fn into_inner(mut self) -> T {
		let mut out = Default::default();
		swap(&mut self.data, &mut out);
		out
	}
}

impl<T> Drop for Protected<T>
where
	T: Zeroize,
{
	fn drop(&mut self) {
		self.data.zeroize();
	}
}

impl<T> Debug for Protected<T>
where
	T: Zeroize,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.write_str("[REDACTED]")
	}
}

#[cfg(feature = "serde")]
impl<'de, T> serde::Deserialize<'de> for Protected<T>
where
	T: serde::Deserialize<'de> + Zeroize,
{
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		Ok(Self::new(T::deserialize(deserializer)?))
	}
}

#[cfg(feature = "rspc")]
impl<T> specta::Type for Protected<T>
where
	T: specta::Type + Zeroize,
{
	const NAME: &'static str = T::NAME;

	fn inline(opts: specta::DefOpts, generics: &[specta::DataType]) -> specta::DataType {
		T::inline(opts, generics)
	}

	fn reference(opts: specta::DefOpts, generics: &[specta::DataType]) -> specta::DataType {
		T::reference(opts, generics)
	}

	fn definition(opts: specta::DefOpts) -> specta::DataType {
		T::definition(opts)
	}
}
