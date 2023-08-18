mod kv;
mod library;

pub use kv::*;
pub use library::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use specta::Type;

use std::collections::HashMap;

use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
#[specta(inline)]
pub struct Settings<V>(V);

impl<V> Preferences for HashMap<Uuid, Settings<V>>
where
	V: Serialize + DeserializeOwned,
{
	fn to_kvs(self) -> PreferenceKVs {
		PreferenceKVs::new(
			self.into_iter()
				.map(|(id, value)| {
					let mut buf = Uuid::encode_buffer();

					let id = id.as_simple().encode_lower(&mut buf);

					(PreferenceKey::new(id), PreferenceValue::new(value))
				})
				.collect(),
		)
	}

	fn from_entries(entries: Entries) -> Self {
		entries
			.into_iter()
			.map(|(key, entry)| (Uuid::parse_str(&key).unwrap(), entry.expect_value()))
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
