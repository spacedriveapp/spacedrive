use crate::{
	api::{
		locations::{file_path_with_object, object_with_file_paths, ExplorerItem},
		utils::library,
	},
	library::{Category, Library},
	location::{
		file_path_helper::{check_file_path_exists, IsolatedFilePathData},
		non_indexed, LocationError,
	},
	object::media::thumbnail::get_indexed_thumb_key,
	prisma::{self, file_path, location, object, tag, tag_on_object, PrismaClient},
};

use std::{collections::BTreeSet, path::PathBuf};

use chrono::{DateTime, FixedOffset, Utc};
use prisma_client_rust::{operator, or, WhereQuery};
use rspc::ErrorCode;
use sd_prisma::prisma::media_data;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{RouterBuilder, R};

const MAX_TAKE: u8 = 100;

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

#[derive(Serialize, Deserialize, Type, Debug, Clone, Copy)]
#[serde(rename_all = "PascalCase")]
pub enum SortOrder {
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
	fn get_sort_order(&self) -> prisma::SortOrder {
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

	fn into_param(self) -> file_path::OrderByWithRelationParam {
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
	with_descendants: Option<bool>,
	#[specta(optional)]
	object: Option<ObjectFilterArgs>,
	#[specta(optional)]
	hidden: Option<bool>,
}

impl FilePathFilterArgs {
	async fn into_params(
		self,
		db: &PrismaClient,
	) -> Result<Vec<file_path::WhereParam>, rspc::Error> {
		let location = if let Some(location_id) = self.location_id {
			Some(
				db.location()
					.find_unique(location::id::equals(location_id))
					.exec()
					.await
					.map_err(|err| {
						rspc::Error::with_cause(
							rspc::ErrorCode::InternalServerError,
							"Internal server error occurred while completing database operation!"
								.into(),
							err,
						)
					})?
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
					Err(rspc::Error::new(
						ErrorCode::NotFound,
						"Directory not found".into(),
					))?;
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
pub struct CursorOrderItem<T> {
	order: SortOrder,
	data: T,
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
	is_dir: bool,
	variant: FilePathCursorVariant,
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum ObjectCursor {
	None,
	DateAccessed(CursorOrderItem<DateTime<FixedOffset>>),
	Kind(CursorOrderItem<i32>),
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase", tag = "field", content = "value")]
pub enum ObjectOrder {
	DateAccessed(SortOrder),
	Kind(SortOrder),
	DateImageTaken(SortOrder),
}

enum MediaDataSortParameter {
	DateImageTaken,
}

impl ObjectOrder {
	fn get_sort_order(&self) -> prisma::SortOrder {
		(*match self {
			Self::DateAccessed(v) => v,
			Self::Kind(v) => v,
			Self::DateImageTaken(v) => v,
		})
		.into()
	}

	fn media_data(
		&self,
		param: MediaDataSortParameter,
		dir: prisma::SortOrder,
	) -> object::OrderByWithRelationParam {
		let order = match param {
			MediaDataSortParameter::DateImageTaken => media_data::epoch_time::order(dir),
		};

		object::media_data::order(vec![order])
	}

	fn into_param(self) -> object::OrderByWithRelationParam {
		let dir = self.get_sort_order();
		use object::*;

		match self {
			Self::DateAccessed(_) => date_accessed::order(dir),
			Self::Kind(_) => kind::order(dir),
			Self::DateImageTaken(_) => self.media_data(MediaDataSortParameter::DateImageTaken, dir),
		}
	}
}

#[derive(Deserialize, Type, Debug)]
#[serde(rename_all = "camelCase")]
pub enum OrderAndPagination<TId, TOrder, TCursor> {
	OrderOnly(TOrder),
	Offset { offset: i32, order: Option<TOrder> },
	Cursor { id: TId, cursor: TCursor },
}

#[derive(Deserialize, Type, Debug, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
enum ObjectHiddenFilter {
	#[default]
	Exclude,
	Include,
}

impl ObjectHiddenFilter {
	fn to_param(self) -> Option<object::WhereParam> {
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

		sd_utils::chain_optional_iter(
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

pub fn mount() -> RouterBuilder {
	R.router()
		.procedure("ephemeralPaths", {
			#[derive(Serialize, Deserialize, Type, Debug, Clone)]
			#[serde(rename_all = "camelCase", tag = "field", content = "value")]
			enum EphemeralPathOrder {
				Name(SortOrder),
				SizeInBytes(SortOrder),
				DateCreated(SortOrder),
				DateModified(SortOrder),
			}

			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			struct EphemeralPathSearchArgs {
				path: PathBuf,
				with_hidden_files: bool,
				#[specta(optional)]
				order: Option<EphemeralPathOrder>,
			}

			R.with(library()).query(
				|(node, library),
				 EphemeralPathSearchArgs {
				     path,
				     with_hidden_files,
				     order,
				 }| async move {
					let mut paths =
						non_indexed::walk(path, with_hidden_files, node, library).await?;

					macro_rules! order_match {
						($order:ident, [$(($variant:ident, |$i:ident| $func:expr)),+]) => {{
							match $order {
								$(EphemeralPathOrder::$variant(order) => {
									paths.entries.sort_unstable_by(|path1, path2| {
										let func = |$i: &ExplorerItem| $func;

										let one = func(path1);
										let two = func(path2);

										match order {
											SortOrder::Desc => two.cmp(&one),
											SortOrder::Asc => one.cmp(&two),
										}
									});
								})+
							}
						}};
					}

					if let Some(order) = order {
						order_match!(
							order,
							[
								(Name, |p| p.name().to_lowercase()),
								(SizeInBytes, |p| p.size_in_bytes()),
								(DateCreated, |p| p.date_created()),
								(DateModified, |p| p.date_modified())
							]
						)
					}

					Ok(paths)
				},
			)
		})
		.procedure("paths", {
			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			struct FilePathSearchArgs {
				#[specta(optional)]
				take: Option<u8>,
				#[specta(optional)]
				order_and_pagination:
					Option<OrderAndPagination<file_path::id::Type, FilePathOrder, FilePathCursor>>,
				#[serde(default)]
				filter: FilePathFilterArgs,
				#[serde(default = "default_group_directories")]
				group_directories: bool,
			}

			fn default_group_directories() -> bool {
				true
			}

			R.with(library()).query(
				|(node, library),
				 FilePathSearchArgs {
				     take,
				     order_and_pagination,
				     filter,
				     group_directories,
				 }| async move {
					let Library { db, .. } = library.as_ref();

					let mut query = db.file_path().find_many(filter.into_params(db).await?);

					if let Some(take) = take {
						query = query.take(take as i64);
					}

					// WARN: this order_by for grouping directories MUST always come before the other order_by
					if group_directories {
						query = query.order_by(file_path::is_dir::order(prisma::SortOrder::Desc));
					}

					// WARN: this order_by for sorting data MUST always come after the other order_by
					if let Some(order_and_pagination) = order_and_pagination {
						match order_and_pagination {
							OrderAndPagination::OrderOnly(order) => {
								query = query.order_by(order.into_param());
							}
							OrderAndPagination::Offset { offset, order } => {
								query = query.skip(offset as i64);

								if let Some(order) = order {
									query = query.order_by(order.into_param())
								}
							}
							OrderAndPagination::Cursor { id, cursor } => {
								// This may seem dumb but it's vital!
								// If we're grouping by directories + all directories have been fetched,
								// we don't want to include them in the results.
								// It's important to keep in mind that since the `order_by` for
								// `group_directories` comes before all other orderings,
								// all other orderings will be applied independently to directories and paths.
								if group_directories && !cursor.is_dir {
									query.add_where(file_path::is_dir::not(Some(true)))
								}

								macro_rules! arm {
									($field:ident, $item:ident) => {{
										let item = $item;

										let data = item.data.clone();

										query.add_where(or![
											match item.order {
												SortOrder::Asc => file_path::$field::gt(data),
												SortOrder::Desc => file_path::$field::lt(data),
											},
											prisma_client_rust::and![
												file_path::$field::equals(Some(item.data)),
												match item.order {
													SortOrder::Asc => file_path::id::gt(id),
													SortOrder::Desc => file_path::id::lt(id),
												}
											]
										]);

										query = query
											.order_by(file_path::$field::order(item.order.into()));
									}};
								}

								match cursor.variant {
									FilePathCursorVariant::None => {
										query.add_where(file_path::id::gt(id));
									}
									FilePathCursorVariant::SizeInBytes(order) => {
										query = query.order_by(
											file_path::size_in_bytes_bytes::order(order.into()),
										);
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
													SortOrder::Asc => file_path::object::is(vec![
														object::$field::gt(item.data),
													]),
													SortOrder::Desc => file_path::object::is(vec![
														object::$field::lt(item.data),
													]),
												});

												query =
													query.order_by(file_path::object::order(vec![
														object::$field::order(item.order.into()),
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

								query =
									query.order_by(file_path::id::order(prisma::SortOrder::Asc));
							}
						}
					}

					let file_paths = query
						.include(file_path_with_object::include())
						.exec()
						.await?;

					let mut items = Vec::with_capacity(file_paths.len());

					for file_path in file_paths {
						let thumbnail_exists_locally = if let Some(cas_id) = &file_path.cas_id {
							library
								.thumbnail_exists(&node, cas_id)
								.await
								.map_err(LocationError::from)?
						} else {
							false
						};

						items.push(ExplorerItem::Path {
							has_local_thumbnail: thumbnail_exists_locally,
							thumbnail_key: file_path
								.cas_id
								.as_ref()
								.map(|i| get_indexed_thumb_key(i, library.id)),
							item: file_path,
						})
					}

					Ok(SearchData {
						items,
						cursor: None,
					})
				},
			)
		})
		.procedure("pathsCount", {
			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			#[specta(inline)]
			struct Args {
				#[serde(default)]
				filter: FilePathFilterArgs,
			}

			R.with(library())
				.query(|(_, library), Args { filter }| async move {
					let Library { db, .. } = library.as_ref();

					Ok(db
						.file_path()
						.count(filter.into_params(db).await?)
						.exec()
						.await? as u32)
				})
		})
		.procedure("objects", {
			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			struct ObjectSearchArgs {
				take: u8,
				#[specta(optional)]
				order_and_pagination:
					Option<OrderAndPagination<object::id::Type, ObjectOrder, ObjectCursor>>,
				#[serde(default)]
				filter: ObjectFilterArgs,
			}

			R.with(library()).query(
				|(node, library),
				 ObjectSearchArgs {
				     take,
				     order_and_pagination,
				     filter,
				 }| async move {
					let Library { db, .. } = library.as_ref();

					let take = take.max(MAX_TAKE);

					let mut query = db
						.object()
						.find_many(filter.into_params())
						.take(take as i64);

					if let Some(order_and_pagination) = order_and_pagination {
						match order_and_pagination {
							OrderAndPagination::OrderOnly(order) => {
								query = query.order_by(order.into_param());
							}
							OrderAndPagination::Offset { offset, order } => {
								query = query.skip(offset as i64);

								if let Some(order) = order {
									query = query.order_by(order.into_param())
								}
							}
							OrderAndPagination::Cursor { id, cursor } => {
								macro_rules! arm {
									($field:ident, $item:ident) => {{
										let item = $item;

										let data = item.data.clone();

										query.add_where(or![
											match item.order {
												SortOrder::Asc => object::$field::gt(data),
												SortOrder::Desc => object::$field::lt(data),
											},
											prisma_client_rust::and![
												object::$field::equals(Some(item.data)),
												match item.order {
													SortOrder::Asc => object::id::gt(id),
													SortOrder::Desc => object::id::lt(id),
												}
											]
										]);

										query = query
											.order_by(object::$field::order(item.order.into()));
									}};
								}

								match cursor {
									ObjectCursor::None => {
										query.add_where(object::id::gt(id));
									}
									ObjectCursor::Kind(item) => arm!(kind, item),
									ObjectCursor::DateAccessed(item) => arm!(date_accessed, item),
								}

								query =
									query.order_by(object::pub_id::order(prisma::SortOrder::Asc))
							}
						}
					}

					let (objects, cursor) = {
						let mut objects = query
							.include(object_with_file_paths::include())
							.exec()
							.await?;

						let cursor = (objects.len() as u8 > take)
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
							library.thumbnail_exists(&node, cas_id).await.map_err(|e| {
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
							thumbnail_key: cas_id.map(|i| get_indexed_thumb_key(i, library.id)),
							item: object,
						});
					}

					Ok(SearchData { items, cursor })
				},
			)
		})
		.procedure("objectsCount", {
			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			#[specta(inline)]
			struct Args {
				#[serde(default)]
				filter: ObjectFilterArgs,
			}

			R.with(library())
				.query(|(_, library), Args { filter }| async move {
					let Library { db, .. } = library.as_ref();

					Ok(db.object().count(filter.into_params()).exec().await? as u32)
				})
		})
}
