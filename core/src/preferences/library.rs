use crate::api::search;
use crate::prisma::PrismaClient;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;
use std::collections::HashMap;
use tracing::error;
use uuid::Uuid;

use super::*;

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPreferences {
	#[serde(default)]
	#[specta(optional)]
	location: HashMap<Uuid, Settings<LocationSettings>>,
}

impl LibraryPreferences {
	pub async fn write(self, db: &PrismaClient) -> prisma_client_rust::Result<()> {
		let kvs = self.to_kvs();

		db._batch(kvs.into_upserts(db)).await?;

		Ok(())
	}

	pub async fn read(db: &PrismaClient) -> prisma_client_rust::Result<Self> {
		let kvs = db.preference().find_many(vec![]).exec().await?;

		let prefs = PreferenceKVs::new(
			kvs.into_iter()
				.filter_map(|data| {
					rmpv::decode::read_value(&mut data.value?.as_slice())
						.map_err(|e| error!("{e:#?}"))
						.ok()
						.map(|value| {
							(
								PreferenceKey::new(data.key),
								PreferenceValue::from_value(value),
							)
						})
				})
				.collect(),
		);

		Ok(prefs.parse())
	}
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LocationSettings {
	explorer: ExplorerSettings<search::FilePathSearchOrdering>,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerSettings<TOrder> {
	layout_mode: Option<ExplorerLayout>,
	grid_item_size: Option<i32>,
	media_columns: Option<i32>,
	media_aspect_square: Option<bool>,
	open_on_double_click: Option<DoubleClickAction>,
	show_bytes_in_grid_view: Option<bool>,
	col_sizes: Option<BTreeMap<String, i32>>,
	// temporary
	#[serde(skip_serializing_if = "Option::is_none")]
	order: Option<Option<TOrder>>,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ExplorerLayout {
	Grid,
	List,
	Media,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum DoubleClickAction {
	OpenFile,
	QuickPreview,
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
