mod kv;

pub use kv::*;
use specta::Type;

use crate::prisma::PrismaClient;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct LibraryPreferences {
	#[serde(default)]
	#[specta(optional)]
	location: HashMap<Uuid, LocationPreferences>,
}

impl LibraryPreferences {
	pub async fn write(self, db: &PrismaClient) -> prisma_client_rust::Result<()> {
		let kvs = self.to_kvs();

		db._batch(kvs.to_upserts(&db)).await?;

		Ok(())
	}
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct LocationPreferences {
	/// View settings for the location - all writes are overwrites!
	#[specta(optional)]
	view: Option<LocationViewSettings>,
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub struct LocationViewSettings {
	layout: ExplorerLayout,
	list: ListViewSettings,
}

#[derive(Clone, Serialize, Deserialize, Type, Default)]
pub struct ListViewSettings {
	#[serde(default, skip_serializing_if = "HashMap::is_empty")]
	col_sizes: HashMap<String, i32>,
	#[serde(default, skip_serializing_if = "Vec::is_empty")]
	filtered: Vec<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	sort_col: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Type)]
pub enum ExplorerLayout {
	Grid,
	List,
	Media,
}

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
}

impl Preferences for LibraryPreferences {
	fn to_kvs(self) -> PreferenceKVs {
		let Self { location } = self;

		location.to_kvs().with_prefix("location")
	}
}

impl Preferences for LocationPreferences {
	fn to_kvs(self) -> PreferenceKVs {
		let Self { view } = self;

		PreferenceKVs::new(
			[view.map(|view| (PreferenceKey::new("view"), PreferenceValue::new(view)))]
				.into_iter()
				.flatten()
				.collect(),
		)
	}
}

pub trait Preferences {
	fn to_kvs(self) -> PreferenceKVs;
}
