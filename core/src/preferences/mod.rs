mod kv;
mod library;

pub use kv::*;
pub use library::*;

use std::collections::HashMap;

use uuid::Uuid;

impl<V> Preferences for HashMap<Uuid, V>
where
	V: Preferences,
{
	fn to_kvs(self) -> PreferenceKVs {
		PreferenceKVs::new(
			self.into_iter()
				.flat_map(|(id, value)| {
					let mut buf = Uuid::encode_buffer();

					let id = id.as_simple().encode_lower(&mut buf);

					value.to_kvs().with_prefix(id)
				})
				.collect(),
		)
	}

	fn from_entries(entries: Entries) -> Self {
		entries
			.into_iter()
			.map(|(key, value)| {
				let id = Uuid::parse_str(&key).unwrap();

				(id, V::from_entries(value.expect_nested()))
			})
			.collect()
	}
}

// Preferences are a set of types that are serialized as a list of key-value pairs,
// where nested type keys are serialized as a dot-separated path.
// They are serailized as a list because this allows preferences to be a synchronisation boundary,
// whereas their values (referred to as settings) will be overwritten.
pub trait Preferences {
	fn to_kvs(self) -> PreferenceKVs;
	fn from_entries(entries: Entries) -> Self;
}
