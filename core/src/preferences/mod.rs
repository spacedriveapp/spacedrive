mod kv;

pub use kv::*;

use crate::prisma::PrismaClient;
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize)]
pub struct LibraryPreferences {
	#[serde(default)]
	location: HashMap<Uuid, LocationPreferences>,
}

impl LibraryPreferences {
	pub async fn write(self, db: &PrismaClient) -> prisma_client_rust::Result<()> {
		let kvs = self.to_kvs();

		db._batch(kvs.to_upserts(&db)).await?;

		Ok(())
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocationPreferences {
	view: Option<LocationViewSettings>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LocationViewSettings {
	layout: Option<ExplorerLayout>,
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

		PreferenceKVs::new(vec![(
			PreferenceKey::new("view"),
			PreferenceValue::new(view),
		)])
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub enum ExplorerLayout {
	Grid,
	List,
	Media,
}

pub trait Preferences {
	fn to_kvs(self) -> PreferenceKVs;
}
