#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use uhlc::NTP64;
use uuid::Uuid;

pub mod db;
pub mod error;

/// Combines an iterator of `T` and an iterator of `Option<T>`,
/// removing any `None` values in the process
pub fn chain_optional_iter<T>(
	required: impl IntoIterator<Item = T>,
	optional: impl IntoIterator<Item = Option<T>>,
) -> Vec<T> {
	required
		.into_iter()
		.map(Some)
		.chain(optional)
		.flatten()
		.collect()
}

/// A splitted version of `u64`, divided into `(u32, u32)`
///
/// rspc/specta doesn't support `BigInt`, so we need this hack
pub type U64Front = (u32, u32);

#[inline]
#[must_use]
pub const fn u64_to_frontend(num: u64) -> U64Front {
	#[allow(clippy::cast_possible_truncation)]
	{
		// SAFETY: We're splitting in (high, low) parts, so we're not going to lose data on truncation
		((num >> 32) as u32, num as u32)
	}
}

/// A splitted version of `i64`, divided into `(i32, u32)`
///
/// rspc/specta doesn't support `BigInt`, so we need this hack
pub type I64Front = (i32, u32);

#[inline]
#[must_use]
pub const fn i64_to_frontend(num: i64) -> I64Front {
	#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
	{
		// SAFETY: We're splitting in (high, low) parts, so we're not going to lose data on truncation
		((num >> 32) as i32, num as u32)
	}
}

#[inline]
#[must_use]
pub fn uuid_to_bytes(uuid: &Uuid) -> Vec<u8> {
	uuid.as_bytes().to_vec()
}

/// Converts a byte slice to a `Uuid`
/// # Panics
/// Panics if the byte slice is not a valid `Uuid` which means we have a corrupted database
#[inline]
#[must_use]
pub fn from_bytes_to_uuid(bytes: &[u8]) -> Uuid {
	Uuid::from_slice(bytes).expect("corrupted uuid in database")
}

#[macro_export]
macro_rules! msgpack {
	(nil) => {
		::rmpv::Value::Nil
	};
	($e:expr) => {{
		let bytes = rmp_serde::to_vec_named(&$e).expect("failed to serialize msgpack");
		let value: rmpv::Value = rmp_serde::from_slice(&bytes).expect("failed to deserialize msgpack");

		value
	}}
}

/// Helper function to convert a [`chrono::DateTime<Utc>`] to a [`uhlc::NTP64`]
#[allow(clippy::missing_panics_doc)] // Doesn't actually panic
#[must_use]
pub fn datetime_to_timestamp(latest_time: DateTime<Utc>) -> NTP64 {
	NTP64::from(
		SystemTime::from(latest_time)
			.duration_since(UNIX_EPOCH)
			.expect("hardcoded earlier time, nothing is earlier than UNIX_EPOCH"),
	)
}

/// Helper function to convert a [`uhlc::NTP64`] to a [`chrono::DateTime<Utc>`]
#[must_use]
pub fn timestamp_to_datetime(timestamp: NTP64) -> DateTime<Utc> {
	DateTime::from(timestamp.to_system_time())
}

// Only used for testing purposes. Do not use in production code.
use std::any::type_name;

#[inline]
#[must_use]
pub fn test_type_of<T>(_: T) -> &'static str {
	type_name::<T>()
}
