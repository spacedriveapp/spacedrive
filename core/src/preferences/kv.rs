use sd_prisma::prisma::{preference, PrismaClient};

use std::collections::BTreeMap;

use itertools::Itertools;
use rmpv::Value;
use serde::{de::DeserializeOwned, Serialize};

use super::Preferences;

#[derive(Debug)]
pub struct PreferenceKey(Vec<String>);

impl PreferenceKey {
	pub fn new(value: impl Into<String>) -> Self {
		Self(
			value
				.into()
				.split('.')
				.map(ToString::to_string)
				.collect_vec(),
		)
	}

	pub fn prepend_path(&mut self, prefix: &str) {
		self.0 = [prefix.to_string()]
			.into_iter()
			.chain(self.0.drain(..))
			.collect_vec();
	}
}

impl std::fmt::Display for PreferenceKey {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0.join("."))
	}
}

#[derive(Debug)]
pub struct PreferenceValue(Vec<u8>);

impl PreferenceValue {
	pub fn new(value: impl Serialize) -> Self {
		let mut bytes = vec![];

		rmp_serde::encode::write_named(&mut bytes, &value)
			.expect("Failed to serialize preference value");

		// let value = rmpv::decode::read_value(&mut bytes.as_slice()).unwrap();

		Self(bytes)
	}

	pub fn from_value(value: Value) -> Self {
		let mut bytes = vec![];

		rmpv::encode::write_value(&mut bytes, &value)
			.expect("Failed to serialize preference value");

		Self(bytes)
	}
}

#[derive(Debug)]
pub struct PreferenceKVs(Vec<(PreferenceKey, PreferenceValue)>);

impl IntoIterator for PreferenceKVs {
	type Item = (PreferenceKey, PreferenceValue);
	type IntoIter = std::vec::IntoIter<Self::Item>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}

#[derive(Debug)]
pub enum Entry {
	Value(Vec<u8>),
	Nested(Entries),
}

#[allow(clippy::unwrap_used, clippy::panic)]
impl Entry {
	pub fn expect_value<T: DeserializeOwned>(self) -> T {
		match self {
			Self::Value(value) => rmp_serde::decode::from_read(value.as_slice()).unwrap(),
			_ => panic!("Expected value"),
		}
	}

	pub fn expect_nested(self) -> Entries {
		match self {
			Self::Nested(entries) => entries,
			_ => panic!("Expected nested entry"),
		}
	}
}

pub type Entries = BTreeMap<String, Entry>;

impl PreferenceKVs {
	pub fn new(values: Vec<(PreferenceKey, PreferenceValue)>) -> Self {
		Self(values)
	}

	pub fn with_prefix(mut self, prefix: &str) -> Self {
		for (key, _) in &mut self.0 {
			key.prepend_path(prefix);
		}

		self
	}

	pub fn into_upserts(self, db: &PrismaClient) -> Vec<preference::UpsertQuery> {
		self.0
			.into_iter()
			.map(|(key, value)| {
				let value = vec![preference::value::set(Some(value.0))];

				db.preference().upsert(
					preference::key::equals(key.to_string()),
					preference::create(key.to_string(), value.clone()),
					value,
				)
			})
			.collect()
	}

	pub fn parse<T: Preferences>(self) -> T {
		let entries = self
			.0
			.into_iter()
			.fold(BTreeMap::new(), |mut acc, (key, value)| {
				let key_parts = key.0;
				let key_parts_len = key_parts.len();

				{
					let mut curr_map: &mut BTreeMap<String, Entry> = &mut acc;

					for (i, part) in key_parts.into_iter().enumerate() {
						if i >= key_parts_len - 1 {
							curr_map.insert(part, Entry::Value(value.0));
							break;
						} else {
							curr_map = match curr_map
								.entry(part)
								.or_insert(Entry::Nested(BTreeMap::new()))
							{
								Entry::Nested(map) => map,
								_ => unreachable!(),
							};
						}
					}
				}

				acc
			});

		T::from_entries(entries)
	}
}
