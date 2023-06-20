use crate::{
	api::{
		locations::{file_path_with_object, object_with_file_paths, ExplorerItem},
		utils::library,
	},
	library::{Category, Library},
	location::{
		file_path_helper::{check_file_path_exists, IsolatedFilePathData},
		find_location, LocationError,
	},
	object::preview::get_thumb_key,
	prisma::{self, file_path, location, object, tag, tag_on_object},
	util::db::chain_optional_iter,
};

use std::collections::BTreeSet;

use chrono::{DateTime, FixedOffset, Utc};
use prisma_client_rust::{operator, or};
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{Ctx, R};

#[derive(Serialize, Type, Debug)]
struct SearchData<T> {
	cursor: Option<Vec<u8>>,
	items: Vec<T>,
}

#[derive(Deserialize, Default, Type, Debug)]
#[serde(rename_all = "camelCase")]
struct OptionalRange<T> {
	from: Option<T>,
	to: Option<T>,
}

#[derive(Deserialize, Type, Debug, Clone, Copy)]
enum SortOrder {
	Asc,
	Desc,
}

impl From<SortOrder> for prisma::SortOrder {
	fn from(value: SortOrder) -> prisma::SortOrder {
		match value {
			SortOrder::Asc => prisma::SortOrder::Asc,
			SortOrder::Desc => prisma::SortOrder::Desc,
		}
	}
}

#[derive(Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
enum FilePathSearchOrdering {
	Name(SortOrder),
	SizeInBytes(SortOrder),
	DateCreated(SortOrder),
	DateModified(SortOrder),
	DateIndexed(SortOrder),
	Object(Box<ObjectSearchOrdering>),
}

impl FilePathSearchOrdering {
	fn get_sort_order(&self) -> prisma::SortOrder {
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

	fn into_param(self) -> file_path::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use file_path::*;
		match self {
			Self::Name(_) => name::order(dir),
			Self::SizeInBytes(_) => size_in_bytes::order(dir),
			Self::DateCreated(_) => date_created::order(dir),
			Self::DateModified(_) => date_modified::order(dir),
			Self::DateIndexed(_) => date_indexed::order(dir),
			Self::Object(v) => object::order(vec![v.into_param()]),
		}
	}
}

#[derive(Deserialize, Type, Debug)]
#[serde(untagged)]
enum MaybeNot<T> {
	None(T),
	Not { not: T },
}

impl<T> MaybeNot<T> {
	fn into_prisma<R: From<prisma_client_rust::Operator<R>>>(self, param: fn(T) -> R) -> R {
		match self {
			Self::None(v) => param(v),
			Self::Not { not } => prisma_client_rust::not![param(not)],
		}
	}
}

#[derive(Deserialize, Type, Default, Debug)]
#[serde(rename_all = "camelCase")]
struct FilePathFilterArgs {
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
	object: Option<ObjectFilterArgs>,
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
struct FilePathSearchArgs {
	#[specta(optional)]
	take: Option<i32>,
	#[specta(optional)]
	order: Option<FilePathSearchOrdering>,
	#[specta(optional)]
	cursor: Option<Vec<u8>>,
	#[serde(default)]
	filter: FilePathFilterArgs,
}

#[derive(Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
enum ObjectSearchOrdering {
	DateAccessed(SortOrder),
}

impl ObjectSearchOrdering {
	fn get_sort_order(&self) -> prisma::SortOrder {
		(*match self {
			Self::DateAccessed(v) => v,
		})
		.into()
	}

	fn into_param(self) -> object::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use object::*;
		match self {
			Self::DateAccessed(_) => date_accessed::order(dir),
		}
	}
}

#[derive(Deserialize, Type, Debug, Default)]
#[serde(rename_all = "camelCase")]
enum ObjectHiddenFilter {
	#[default]
	Exclude,
	Include,
}

impl ObjectHiddenFilter {
	fn to_param(&self) -> Option<object::WhereParam> {
		match self {
			ObjectHiddenFilter::Exclude => Some(or![
				object::hidden::equals(None),
				object::hidden::not(Some(true))
			]),
			ObjectHiddenFilter::Include => None,
		}
	}
}

#[derive(Deserialize, Type, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct ObjectFilterArgs {
	#[specta(optional)]
	favorite: Option<bool>,
	#[serde(default)]
	hidden: ObjectHiddenFilter,
	#[specta(optional)]
	date_accessed: Option<MaybeNot<Option<chrono::DateTime<FixedOffset>>>>,
	#[serde(default)]
	kind: BTreeSet<i32>,
	#[serde(default)]
	tags: Vec<i32>,
	#[specta(optional)]
	category: Option<Category>,
}

impl ObjectFilterArgs {
	fn into_params(self) -> Vec<object::WhereParam> {
		use object::*;

		chain_optional_iter(
			[],
			[
				self.hidden.to_param(),
				self.favorite.map(Some).map(favorite::equals),
				self.date_accessed
					.map(|date| date.into_prisma(date_accessed::equals)),
				(!self.kind.is_empty()).then(|| kind::in_vec(self.kind.into_iter().collect())),
				(!self.tags.is_empty()).then(|| {
					let tags = self.tags.into_iter().map(tag::id::equals).collect();
					let tags_on_object = tag_on_object::tag::is(vec![operator::or(tags)]);

					tags::some(vec![tags_on_object])
				}),
				self.category.map(Category::to_where_param),
			],
		)
	}
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
struct ObjectSearchArgs {
	#[specta(optional)]
	take: Option<i32>,
	#[specta(optional)]
	order: Option<ObjectSearchOrdering>,
	#[specta(optional)]
	cursor: Option<Vec<u8>>,
	#[serde(default)]
	filter: ObjectFilterArgs,
}

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("paths", {
			R.with2(library()).query(
				|(_, library),
				 FilePathSearchArgs {
				     take,
				     order,
				     cursor,
				     filter,
				 }| async move {
					let Library { db, .. } = &library;

					let location = if let Some(location_id) = filter.location_id {
						Some(
							find_location(&library, location_id)
								.exec()
								.await?
								.ok_or(LocationError::IdNotFound(location_id))?,
						)
					} else {
						None
					};

					let directory_materialized_path_str = match (filter.path, location) {
						(Some(path), Some(location)) if !path.is_empty() && path != "/" => {
							let parent_iso_file_path =
								IsolatedFilePathData::from_relative_str(location.id, &path);
							if !check_file_path_exists::<LocationError>(&parent_iso_file_path, db)
								.await?
							{
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

					use file_path::*;

					let params = chain_optional_iter(
						filter
							.search
							.unwrap_or_default()
							.split(' ')
							.map(str::to_string)
							.map(name::contains),
						[
							filter.location_id.map(Some).map(location_id::equals),
							filter.extension.map(Some).map(extension::equals),
							filter.created_at.from.map(|v| date_created::gte(v.into())),
							filter.created_at.to.map(|v| date_created::lte(v.into())),
							directory_materialized_path_str
								.map(Some)
								.map(materialized_path::equals),
							filter.object.and_then(|obj| {
								let params = obj.into_params();

								(!params.is_empty()).then(|| object::is(params))
							}),
						],
					);

					let take = take.unwrap_or(100);

					let mut query = db.file_path().find_many(params).take(take as i64 + 1);

					if let Some(order) = order {
						query = query.order_by(order.into_param());
					}

					if let Some(cursor) = cursor {
						query = query.cursor(file_path::pub_id::equals(cursor));
					}

					let (file_paths, cursor) = {
						let mut paths = query
							.include(file_path_with_object::include())
							.exec()
							.await?;

						let cursor = (paths.len() as i32 > take)
							.then(|| paths.pop())
							.flatten()
							.map(|r| r.pub_id);

						(paths, cursor)
					};

					let mut items = Vec::with_capacity(file_paths.len());

					for file_path in file_paths {
						let thumbnail_exists_locally = if let Some(cas_id) = &file_path.cas_id {
							library
								.thumbnail_exists(cas_id)
								.await
								.map_err(LocationError::from)?
						} else {
							false
						};

						items.push(ExplorerItem::Path {
							has_local_thumbnail: thumbnail_exists_locally,
							thumbnail_key: file_path.cas_id.as_ref().map(|i| get_thumb_key(i)),
							item: file_path,
						})
					}

					Ok(SearchData { items, cursor })
				},
			)
		})
		.procedure("objects", {
			R.with2(library()).query(
				|(_, library),
				 ObjectSearchArgs {
				     take,
				     order,
				     cursor,
				     filter,
				 }| async move {
					let Library { db, .. } = &library;

					let take = take.unwrap_or(100);

					let mut query = db
						.object()
						.find_many(filter.into_params())
						.take(take as i64 + 1);

					if let Some(order) = order {
						query = query.order_by(order.into_param());
					}

					if let Some(cursor) = cursor {
						query = query.cursor(object::pub_id::equals(cursor));
					}

					let (objects, cursor) = {
						let mut objects = query
							.include(object_with_file_paths::include())
							.exec()
							.await?;

						let cursor = (objects.len() as i32 > take)
							.then(|| objects.pop())
							.flatten()
							.map(|r| r.pub_id);

						(objects, cursor)
					};

					let mut items = Vec::with_capacity(objects.len());

					for object in objects {
						let cas_id = object
							.file_paths
							.iter()
							.map(|fp| fp.cas_id.as_ref())
							.find_map(|c| c);

						let thumbnail_exists_locally = if let Some(cas_id) = cas_id {
							library.thumbnail_exists(cas_id).await.map_err(|e| {
								rspc::Error::with_cause(
									ErrorCode::InternalServerError,
									"Failed to check that thumbnail exists".to_string(),
									e,
								)
							})?
						} else {
							false
						};

						items.push(ExplorerItem::Object {
							has_local_thumbnail: thumbnail_exists_locally,
							thumbnail_key: cas_id.map(|i| get_thumb_key(i)),
							item: object,
						});
					}

					Ok(SearchData { items, cursor })
				},
			)
		})
}
