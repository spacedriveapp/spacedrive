mod kv;

pub use kv::*;
use specta::Type;

use crate::prisma::PrismaClient;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
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

	pub async fn read(db: &PrismaClient) -> prisma_client_rust::Result<Self> {
		let kvs = db.preference().find_many(vec![]).exec().await?;

		let prefs = PreferenceKVs::new(
			kvs.into_iter()
				.filter_map(|data| {
					let a = rmpv::decode::read_value(&mut data.value?.as_slice()).unwrap();

					Some((PreferenceKey::new(data.key), PreferenceValue::from_value(a)))
				})
				.collect(),
		);

		Ok(prefs.parse())
	}
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct LocationPreferences {
	/// View settings for the location - all writes are overwrites!
	#[specta(optional)]
	view: Option<LocationViewSettings>,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct LocationViewSettings {
	layout: ExplorerLayout,
	list: ListViewSettings,
}

#[derive(Clone, Serialize, Deserialize, Type, Default, Debug)]
pub struct ListViewSettings {
	columns: HashMap<String, ListViewColumnSettings>,
	sort_col: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Type, Default, Debug)]
pub struct ListViewColumnSettings {
	hide: bool,
	size: Option<i32>,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
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

impl Preferences for LibraryPreferences {
	fn to_kvs(self) -> PreferenceKVs {
		let Self { location } = self;

		location.to_kvs().with_prefix("location")
	}

	fn from_entries(mut entries: Entries) -> Self {
		Self {
			location: entries
				.remove("location")
				.map(|value| HashMap::from_entries(value.expect_nested()))
				.unwrap_or_default(),
		}
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

	fn from_entries(mut entries: Entries) -> Self {
		Self {
			view: entries.remove("view").map(|view| view.expect_value()),
		}
	}
}

pub trait Preferences {
	fn to_kvs(self) -> PreferenceKVs;
	fn from_entries(entries: Entries) -> Self;
}
