use chrono::{DateTime, FixedOffset, Utc};
use prisma_client_rust::OrderByQuery;
use prisma_client_rust::PaginatedQuery;
use prisma_client_rust::WhereQuery;
use rspc::ErrorCode;
use sd_prisma::prisma::{self, file_path, location};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::location::file_path_helper::check_file_path_exists;
use crate::location::file_path_helper::IsolatedFilePathData;
use crate::location::LocationError;

use super::object::*;
use super::utils::{self, *};

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
	locations: Option<InOrNotIn<file_path::id::Type>>,
	#[specta(optional)]
	search: Option<String>,
	#[specta(optional)]
	extension: Option<InOrNotIn<String>>,
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
		let location_conditions = self.locations.clone().and_then(|v| {
			v.to_param(
				file_path::location_id::in_vec,
				file_path::location_id::not_in_vec,
			)
		});

		let first_location_id = if let Some(InOrNotIn::In(location_ids)) = &self.locations {
			location_ids.first().copied()
		} else {
			None
		};

		let directory_materialized_path_str = match (self.path, first_location_id) {
			(Some(path), Some(first_location_id)) if !path.is_empty() && path != "/" => {
				let parent_iso_file_path =
					IsolatedFilePathData::from_relative_str(first_location_id, &path);

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
					location_conditions,
					self.extension
						.and_then(|v| v.to_param(extension::in_vec, extension::not_in_vec)),
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

pub type OrderAndPagination =
	utils::OrderAndPagination<prisma::file_path::id::Type, FilePathOrder, FilePathCursor>;

impl OrderAndPagination {
	pub fn apply(self, query: &mut file_path::FindManyQuery, group_directories: bool) {
		match self {
			Self::OrderOnly(order) => {
				query.add_order_by(order.into_param());
			}
			Self::Offset { offset, order } => {
				query.set_skip(offset as i64);

				if let Some(order) = order {
					query.add_order_by(order.into_param())
				}
			}
			Self::Cursor { id, cursor } => {
				// This may seem dumb but it's vital!
				// If we're grouping by directories + all directories have been fetched,
				// we don't want to include them in the results.
				// It's important to keep in mind that since the `order_by` for
				// `group_directories` comes before all other orderings,
				// all other orderings will be applied independently to directories and paths.
				if group_directories && !cursor.is_dir {
					query.add_where(prisma::file_path::is_dir::not(Some(true)))
				}

				macro_rules! arm {
					($field:ident, $item:ident) => {{
						let item = $item;

						let data = item.data.clone();

						query.add_where(prisma_client_rust::or![
							match item.order {
								SortOrder::Asc => prisma::file_path::$field::gt(data),
								SortOrder::Desc => prisma::file_path::$field::lt(data),
							},
							prisma_client_rust::and![
								prisma::file_path::$field::equals(Some(item.data)),
								match item.order {
									SortOrder::Asc => prisma::file_path::id::gt(id),
									SortOrder::Desc => prisma::file_path::id::lt(id),
								}
							]
						]);

						query.add_order_by(prisma::file_path::$field::order(item.order.into()));
					}};
				}

				match cursor.variant {
					FilePathCursorVariant::None => {
						query.add_where(prisma::file_path::id::gt(id));
					}
					FilePathCursorVariant::SizeInBytes(order) => {
						query.add_order_by(prisma::file_path::size_in_bytes_bytes::order(
							order.into(),
						));
					}
					FilePathCursorVariant::Name(item) => arm!(name, item),
					FilePathCursorVariant::DateCreated(item) => {
						arm!(date_created, item)
					}
					FilePathCursorVariant::DateModified(item) => {
						arm!(date_modified, item)
					}
					FilePathCursorVariant::DateIndexed(item) => {
						arm!(date_indexed, item)
					}
					FilePathCursorVariant::Object(obj) => {
						macro_rules! arm {
							($field:ident, $item:ident) => {{
								let item = $item;

								query.add_where(match item.order {
									SortOrder::Asc => prisma::file_path::object::is(vec![
										prisma::object::$field::gt(item.data),
									]),
									SortOrder::Desc => prisma::file_path::object::is(vec![
										prisma::object::$field::lt(item.data),
									]),
								});

								query.add_order_by(prisma::file_path::object::order(vec![
									prisma::object::$field::order(item.order.into()),
								]));
							}};
						}

						match obj {
							FilePathObjectCursor::Kind(item) => arm!(kind, item),
							FilePathObjectCursor::DateAccessed(item) => {
								arm!(date_accessed, item)
							}
						};
					}
				};

				query.add_order_by(prisma::file_path::id::order(prisma::SortOrder::Asc));
			}
		}
	}
}
