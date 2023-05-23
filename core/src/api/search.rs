use crate::location::file_path_helper::{check_file_path_exists, IsolatedFilePathData};
use std::collections::BTreeSet;

use chrono::{DateTime, FixedOffset, Utc};
use prisma_client_rust::operator::or;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{
	api::{
		locations::{file_path_with_object, object_with_file_paths, ExplorerItem},
		utils::library,
	},
	library::Library,
	location::{find_location, LocationError},
	prisma::{self, file_path, object, tag, tag_on_object},
	util::db::chain_optional_iter,
};

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

impl Into<prisma::SortOrder> for SortOrder {
	fn into(self) -> prisma::SortOrder {
		match self {
			Self::Asc => prisma::SortOrder::Asc,
			Self::Desc => prisma::SortOrder::Desc,
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

	fn to_param(self) -> file_path::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use file_path::*;
		match self {
			Self::Name(_) => name::order(dir),
			Self::SizeInBytes(_) => size_in_bytes::order(dir),
			Self::DateCreated(_) => date_created::order(dir),
			Self::DateModified(_) => date_modified::order(dir),
			Self::DateIndexed(_) => date_indexed::order(dir),
			Self::Object(v) => object::order(vec![v.to_param()]),
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
	fn to_prisma<R: From<prisma_client_rust::Operator<R>>>(self, param: fn(T) -> R) -> R {
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
	location_id: Option<i32>,
	#[serde(default)]
	search: String,
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

	fn to_param(self) -> object::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use object::*;
		match self {
			Self::DateAccessed(_) => date_accessed::order(dir),
		}
	}
}

#[derive(Deserialize, Type, Debug, Default)]
#[serde(rename_all = "camelCase")]
struct ObjectFilterArgs {
	#[specta(optional)]
	favorite: Option<bool>,
	#[specta(optional)]
	hidden: Option<bool>,
	#[specta(optional)]
	date_accessed: Option<MaybeNot<Option<chrono::DateTime<FixedOffset>>>>,
	#[serde(default)]
	kind: BTreeSet<i32>,
	#[serde(default)]
	tags: Vec<i32>,
}

impl ObjectFilterArgs {
	fn to_params(self) -> Vec<object::WhereParam> {
		chain_optional_iter(
			[],
			[
				self.favorite.map(object::favorite::equals),
				self.hidden.map(object::hidden::equals),
				self.date_accessed
					.map(|date| date.to_prisma(object::date_accessed::equals)),
				(!self.kind.is_empty())
					.then(|| object::kind::in_vec(self.kind.into_iter().collect())),
				(!self.tags.is_empty()).then(|| {
					let tags = self.tags.into_iter().map(tag::id::equals).collect();
					let tags_on_object = tag_on_object::tag::is(vec![or(tags)]);

					object::tags::some(vec![tags_on_object])
				}),
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

					let params = chain_optional_iter(
						filter
							.search
							.split(' ')
							.map(str::to_string)
							.map(file_path::name::contains),
						[
							filter.location_id.map(file_path::location_id::equals),
							filter.extension.map(file_path::extension::equals),
							filter
								.created_at
								.from
								.map(|v| file_path::date_created::gte(v.into())),
							filter
								.created_at
								.to
								.map(|v| file_path::date_created::lte(v.into())),
							directory_materialized_path_str
								.map(file_path::materialized_path::equals),
							filter.object.and_then(|obj| {
								let params = obj.to_params();

								(!params.is_empty()).then(|| file_path::object::is(params))
							}),
						],
					);

					let take = take.unwrap_or(100);

					let mut query = db.file_path().find_many(params).take(take as i64 + 1);

					if let Some(order) = order {
						query = query.order_by(order.to_param());
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
						let has_thumbnail = if let Some(cas_id) = &file_path.cas_id {
							library
								.thumbnail_exists(cas_id)
								.await
								.map_err(LocationError::from)?
						} else {
							false
						};

						items.push(ExplorerItem::Path {
							has_thumbnail,
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
						.find_many(filter.to_params())
						.take(take as i64 + 1);

					if let Some(order) = order {
						query = query.order_by(order.to_param());
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

						let has_thumbnail = if let Some(cas_id) = cas_id {
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
							has_thumbnail,
							item: object,
						});
					}

					Ok(SearchData { items, cursor })
				},
			)
		})
}
