//! Copied from: https://docs.rs/async-graphql/latest/async_graphql/types/enum.MaybeUndefined.html
#![allow(unused)]

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use specta::Type;

#[derive(Debug, Clone, Type)]
#[specta(untagged)]
pub enum MaybeUndefined<T> {
	Undefined,
	Null,
	Value(T),
}

impl<T, E> MaybeUndefined<Result<T, E>> {
	/// Transposes a `MaybeUndefined` of a [`Result`] into a [`Result`] of a
	/// `MaybeUndefined`.
	///
	/// [`MaybeUndefined::Undefined`] will be mapped to
	/// [`Ok`]`(`[`MaybeUndefined::Undefined`]`)`. [`MaybeUndefined::Null`]
	/// will be mapped to [`Ok`]`(`[`MaybeUndefined::Null`]`)`.
	/// [`MaybeUndefined::Value`]`(`[`Ok`]`(_))` and
	/// [`MaybeUndefined::Value`]`(`[`Err`]`(_))` will be mapped to
	/// [`Ok`]`(`[`MaybeUndefined::Value`]`(_))` and [`Err`]`(_)`.
	#[inline]
	pub fn transpose(self) -> Result<MaybeUndefined<T>, E> {
		match self {
			MaybeUndefined::Undefined => Ok(MaybeUndefined::Undefined),
			MaybeUndefined::Null => Ok(MaybeUndefined::Null),
			MaybeUndefined::Value(Ok(v)) => Ok(MaybeUndefined::Value(v)),
			MaybeUndefined::Value(Err(e)) => Err(e),
		}
	}
}

impl<T: Serialize> Serialize for MaybeUndefined<T> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		match self {
			MaybeUndefined::Value(value) => value.serialize(serializer),
			_ => serializer.serialize_none(),
		}
	}
}

impl<'de, T> Deserialize<'de> for MaybeUndefined<T>
where
	T: Deserialize<'de>,
{
	fn deserialize<D>(deserializer: D) -> Result<MaybeUndefined<T>, D::Error>
	where
		D: Deserializer<'de>,
	{
		Option::<T>::deserialize(deserializer).map(|value| match value {
			Some(value) => MaybeUndefined::Value(value),
			None => MaybeUndefined::Null,
		})
	}
}
