use crate::location::LocationError;

use sd_core_file_path_helper::{check_file_path_exists, IsolatedFilePathData};

use sd_prisma::prisma::{self, file_path};

use chrono::{DateTime, FixedOffset, Utc};
use prisma_client_rust::{OrderByQuery, PaginatedQuery, WhereQuery};
use rspc::ErrorCode;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{
	object::*,
	utils::{self, *},
};

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum FilePathOrder {
	Name(SortOrder),
	SizeInBytes(SortOrder),
	DateCreated(SortOrder),
	DateModified(SortOrder),
	DateIndexed(SortOrder),
	Object(Box<ObjectOrder>),
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
		}
	}
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FilePathFilterArgs {
	Locations(InOrNotIn<file_path::id::Type>),
	Path {
		location_id: prisma::location::id::Type,
		path: String,
		include_descendants: bool,
	},
	// #[deprecated]
	// Search(String),
	Name(TextMatch),
	Extension(InOrNotIn<String>),
	CreatedAt(Range<DateTime<Utc>>),
	ModifiedAt(Range<DateTime<Utc>>),
	IndexedAt(Range<DateTime<Utc>>),
	Hidden(bool),
}

impl FilePathFilterArgs {
	pub async fn into_params(
		self,
		db: &prisma::PrismaClient,
	) -> Result<Vec<file_path::WhereParam>, rspc::Error> {
		use file_path::*;

		Ok(match self {
			Self::Locations(v) => v
				.into_param(
					file_path::location_id::in_vec,
					file_path::location_id::not_in_vec,
				)
				.map(|v| vec![v])
				.unwrap_or_default(),
			Self::Path {
				location_id,
				path,
				include_descendants,
			} => {
				let directory_materialized_path_str = if !path.is_empty() && path != "/" {
					let parent_iso_file_path =
						IsolatedFilePathData::from_relative_str(location_id, &path);

					if !check_file_path_exists::<LocationError>(&parent_iso_file_path, db).await? {
						return Err(rspc::Error::new(
							ErrorCode::NotFound,
							"Directory not found".into(),
						));
					}

					parent_iso_file_path.materialized_path_for_children()
				} else {
					Some("/".into())
				};

				directory_materialized_path_str
					.map(Some)
					.map(|materialized_path| {
						vec![if include_descendants {
							materialized_path::starts_with(
								materialized_path.unwrap_or_else(|| "/".into()),
							)
						} else {
							materialized_path::equals(materialized_path)
						}]
					})
					.unwrap_or_default()
			}
			Self::Name(v) => v
				.into_param(name::contains, name::starts_with, name::ends_with, |s| {
					name::equals(Some(s))
				})
				.map(|v| vec![v])
				.unwrap_or_default(),
			Self::Extension(v) => v
				.into_param(extension::in_vec, extension::not_in_vec)
				.map(|v| vec![v])
				.unwrap_or_default(),
			Self::CreatedAt(v) => {
				vec![match v {
					Range::From(v) => date_created::gte(v.into()),
					Range::To(v) => date_created::lte(v.into()),
				}]
			}
			Self::ModifiedAt(v) => {
				vec![match v {
					Range::From(v) => date_modified::gte(v.into()),
					Range::To(v) => date_modified::lte(v.into()),
				}]
			}
			Self::IndexedAt(v) => {
				vec![match v {
					Range::From(v) => date_indexed::gte(v.into()),
					Range::To(v) => date_indexed::lte(v.into()),
				}]
			}
			Self::Hidden(v) => {
				vec![hidden::equals(Some(v))]
			}
		})
	}
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum FilePathObjectCursor {
	DateAccessed(CursorOrderItem<DateTime<FixedOffset>>),
	Kind(CursorOrderItem<i32>),
}

impl FilePathObjectCursor {
	fn apply(self, query: &mut file_path::FindManyQuery) {
		macro_rules! arm {
			($field:ident, $item:ident) => {{
				let item = $item;

				query.add_where(match item.order {
					SortOrder::Asc => {
						prisma::file_path::object::is(vec![prisma::object::$field::gt(item.data)])
					}
					SortOrder::Desc => {
						prisma::file_path::object::is(vec![prisma::object::$field::lt(item.data)])
					}
				});

				query.add_order_by(prisma::file_path::object::order(vec![
					prisma::object::$field::order(item.order.into()),
				]));
			}};
		}

		match self {
			FilePathObjectCursor::Kind(item) => arm!(kind, item),
			FilePathObjectCursor::DateAccessed(item) => {
				arm!(date_accessed, item)
			}
		};
	}
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

impl FilePathCursorVariant {
	pub fn apply(self, query: &mut file_path::FindManyQuery, id: i32) {
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

		match self {
			Self::None => {
				query.add_where(prisma::file_path::id::gt(id));
			}
			Self::SizeInBytes(order) => {
				query.add_order_by(prisma::file_path::size_in_bytes_bytes::order(order.into()));
			}
			Self::Name(item) => arm!(name, item),
			Self::DateCreated(item) => {
				arm!(date_created, item)
			}
			Self::DateModified(item) => {
				arm!(date_modified, item)
			}
			Self::DateIndexed(item) => {
				arm!(date_indexed, item)
			}
			Self::Object(obj) => obj.apply(query),
		};
	}
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

				cursor.variant.apply(query, id);

				query.add_order_by(prisma::file_path::id::order(prisma::SortOrder::Asc));
			}
		}
	}
}
