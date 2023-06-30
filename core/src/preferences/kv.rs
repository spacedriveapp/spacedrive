use crate::prisma::{preference, PrismaClient};
use serde::Serialize;

#[derive(Debug)]
pub struct PreferenceKey(String);

impl PreferenceKey {
	pub fn new(value: impl Into<String>) -> Self {
		Self(value.into())
	}

	pub fn prepend_path(&mut self, prefix: &str) {
		self.0 = format!("{}.{}", prefix, self.0);
	}
}

impl std::fmt::Display for PreferenceKey {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

#[derive(Debug)]
pub struct PreferenceValue(Vec<u8>);

impl PreferenceValue {
	pub fn new(value: impl Serialize) -> Self {
		let mut bytes = vec![];

		rmp_serde::encode::write(&mut bytes, &value).unwrap();

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

	pub fn to_upserts(self, db: &PrismaClient) -> Vec<preference::UpsertQuery> {
		self.0
			.into_iter()
			.map(|(key, value)| {
				let value = vec![preference::value::set(Some(value.0))];

				db.preference().upsert(
					preference::key::equals(key.0.clone()),
					preference::create(key.0, value.clone()),
					value,
				)
			})
			.collect()
	}
}
