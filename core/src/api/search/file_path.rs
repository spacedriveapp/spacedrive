use chrono::{DateTime, FixedOffset, Utc};
use rspc::ErrorCode;
use sd_prisma::prisma::{self, file_path, location};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::location::file_path_helper::check_file_path_exists;
use crate::location::file_path_helper::IsolatedFilePathData;
use crate::location::LocationError;

use super::object::*;
use super::utils::*;

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum FilePathOrder {
	Name(SortOrder),
	SizeInBytes(SortOrder),
	DateCreated(SortOrder),
	DateModified(SortOrder),
	DateIndexed(SortOrder),
	Object(Box<ObjectOrder>),
	DateImageTaken(Box<ObjectOrder>),
}

impl FilePathOrder {
	pub fn get_sort_order(&self) -> prisma::SortOrder {
		(*match self {
			Self::Name(v) => v,
			Self::SizeInBytes(v) => v,
			Self::DateCreated(v) => v,
			Self::DateModified(v) => v,
			Self::DateIndexed(v) => v,
			Self::Object(v) => return v.get_sort_order(),
			Self::DateImageTaken(v) => return v.get_sort_order(),
		})
		.into()
	}

	pub fn into_param(self) -> file_path::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use file_path::*;
		match self {
			Self::Name(_) => name::order(dir),
			Self::SizeInBytes(_) => size_in_bytes_bytes::order(dir),
			Self::DateCreated(_) => date_created::order(dir),
			Self::DateModified(_) => date_modified::order(dir),
			Self::DateIndexed(_) => date_indexed::order(dir),
			Self::Object(v) => object::order(vec![v.into_param()]),
			Self::DateImageTaken(v) => object::order(vec![v.into_param()]),
		}
	}
}

#[derive(Deserialize, Type, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FilePathFilterArgs {
	#[specta(optional)]
	location_id: Option<location::id::Type>,
	#[specta(optional)]
	search: Option<String>,
	#[specta(optional)]
	extension: Option<String>,
	#[serde(default)]
	created_at: OptionalRange<DateTime<Utc>>,
	#[specta(optional)]
	path: Option<String>,
	#[specta(optional)]
	with_descendants: Option<bool>,
	#[specta(optional)]
	object: Option<ObjectFilterArgs>,
	#[specta(optional)]
	hidden: Option<bool>,
}

impl FilePathFilterArgs {
	pub async fn into_params(
		self,
		db: &prisma::PrismaClient,
	) -> Result<Vec<file_path::WhereParam>, rspc::Error> {
		let location = if let Some(location_id) = self.location_id {
			Some(
				db.location()
					.find_unique(location::id::equals(location_id))
					.exec()
					.await?
					.ok_or(LocationError::IdNotFound(location_id))?,
			)
		} else {
			None
		};

		let directory_materialized_path_str = match (self.path, location) {
			(Some(path), Some(location)) if !path.is_empty() && path != "/" => {
				let parent_iso_file_path =
					IsolatedFilePathData::from_relative_str(location.id, &path);

				if !check_file_path_exists::<LocationError>(&parent_iso_file_path, db).await? {
					return Err(rspc::Error::new(
						ErrorCode::NotFound,
						"Directory not found".into(),
					));
				}

				parent_iso_file_path.materialized_path_for_children()
			}
			(Some(_empty), _) => Some("/".into()),
			_ => None,
		};

		{
			use file_path::*;

			Ok(sd_utils::chain_optional_iter(
				self.search
					.unwrap_or_default()
					.split(' ')
					.map(str::to_string)
					.map(name::contains),
				[
					self.location_id.map(Some).map(location_id::equals),
					self.extension.map(Some).map(extension::equals),
					self.created_at.from.map(|v| date_created::gte(v.into())),
					self.created_at.to.map(|v| date_created::lte(v.into())),
					self.hidden.map(Some).map(hidden::equals),
					directory_materialized_path_str
						.map(Some)
						.map(|materialized_path| {
							if let Some(true) = self.with_descendants {
								materialized_path::starts_with(
									materialized_path.unwrap_or_else(|| "/".into()),
								)
							} else {
								materialized_path::equals(materialized_path)
							}
						}),
					self.object.and_then(|obj| {
						let params = obj.into_params();

						(!params.is_empty()).then(|| object::is(params))
					}),
				],
			))
		}
	}
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum FilePathObjectCursor {
	DateAccessed(CursorOrderItem<DateTime<FixedOffset>>),
	Kind(CursorOrderItem<i32>),
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum FilePathCursorVariant {
	None,
	Name(CursorOrderItem<String>),
	SizeInBytes(SortOrder),
	DateCreated(CursorOrderItem<DateTime<FixedOffset>>),
	DateModified(CursorOrderItem<DateTime<FixedOffset>>),
	DateIndexed(CursorOrderItem<DateTime<FixedOffset>>),
	Object(FilePathObjectCursor),
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FilePathCursor {
	pub is_dir: bool,
	pub variant: FilePathCursorVariant,
}
