use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct LibraryPreferences {
	#[serde(default)]
	location: HashMap<Uuid, LocationPreferences>,
}

#[derive(Serialize, Deserialize)]
pub struct LocationPreferences {
	view: Option<LocationViewSettings>,
}

#[derive(Serialize, Deserialize)]
pub struct LocationViewSettings {
	layout: Option<ExplorerLayout>,
}

impl<V> Preferences for HashMap<Uuid, V>
where
	V: Preferences,
{
	fn to_kvs(self) -> PreferenceKVs {
		self.into_iter()
			.flat_map(|(id, value)| {
				value.to_kvs().into_iter().map(|(key, value)| {
					let mut id_string = format!("");

					id.as_simple().encode_lower(&mut id_string);

					(format!("{}.{}", id_string, key.0), value)
				})
			})
			.collect()
	}
}

impl Preferences for LibraryPreferences {
	fn to_kvs(self) -> PreferenceKVs {
		let Self { location } = self;

		location
			.to_kvs()
			.into_iter()
			.map(|(key, value)| (format!("location.{}", key.0), value))
			.collect()
	}
}

pub enum ExplorerLayout {
	Grid,
	List,
	Media,
}

pub struct PreferenceKey(String);
pub struct PreferenceValue(rmp_value::Value);

pub type PreferenceKVs = Vec<(PreferenceKey, PreferenceValue)>;

pub trait Preferences {
	fn to_kvs(self) -> PreferenceKVs;
}
