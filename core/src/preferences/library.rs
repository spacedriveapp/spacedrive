use crate::prisma::{PrismaClient, SortOrder};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;
use std::collections::HashMap;
use uuid::Uuid;

use super::*;

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct LibraryPreferences {
	#[serde(default)]
	#[specta(optional)]
	location: HashMap<Uuid, LocationPreferences>,
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
	#[serde(skip_serializing_if = "Option::is_none")]
	view: Option<LocationViewSettings>,
	#[serde(skip_serializing_if = "Option::is_none")]
	list: Option<ListViewSettings>,
	#[serde(skip_serializing_if = "Option::is_none")]
	media: Option<MediaViewSettings>,
	#[serde(skip_serializing_if = "Option::is_none")]
	grid: Option<GridViewSettings>,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct LocationViewSettings {
	layout: Option<ExplorerLayout>,
}

#[derive(Clone, Serialize, Deserialize, Type, Default, Debug)]
pub struct ListViewSettings {
	double_click_action: Option<bool>,
	sort_by: Option<ViewSortBy>,
	col_sizes: Option<BTreeMap<i32, i32>>,
	#[specta(type = _SortOrderType, inline)]
	direction: Option<SortOrder>,
}

#[derive(Type)]
pub enum _SortOrderType {
	#[serde(rename = "Asc")]
	_Asc,
	#[serde(rename = "Desc")]
	_Desc,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct MediaViewSettings {
	item_size: Option<i32>,
	sort_by: Option<ViewSortBy>,
	#[specta(type = _SortOrderType, inline)]
	direction: Option<SortOrder>,
	double_click_action: Option<bool>,
	show_square_thumbnails: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct GridViewSettings {
	item_size: Option<i32>,
	sort_by: Option<ViewSortBy>,
	#[specta(type = _SortOrderType, inline)]
	direction: Option<SortOrder>,
	double_click_action: Option<bool>,
	show_object_size: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub enum ViewSortBy {
	None,
	Name,
	Size,
	DateCreated,
	DateModified,
	DateIndexed,
	DateAccessed,
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub enum ExplorerLayout {
	Grid,
	List,
	Media,
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
		let Self {
			view,
			list,
			media,
			grid,
		} = self;

		PreferenceKVs::new(
			[
				view.map(|v| (PreferenceKey::new("view"), PreferenceValue::new(v))),
				list.map(|v| (PreferenceKey::new("list"), PreferenceValue::new(v))),
				media.map(|v| (PreferenceKey::new("media"), PreferenceValue::new(v))),
				grid.map(|v| (PreferenceKey::new("grid"), PreferenceValue::new(v))),
			]
			.into_iter()
			.flatten()
			.collect(),
		)
	}

	fn from_entries(mut entries: Entries) -> Self {
		Self {
			view: entries.remove("view").map(|view| view.expect_value()),
			list: entries.remove("list").map(|list| list.expect_value()),
			media: entries.remove("media").map(|media| media.expect_value()),
			grid: entries.remove("grid").map(|grid| grid.expect_value()),
		}
	}
}
