mod file_path;
mod media_data;
mod object;
mod utils;

pub use self::{file_path::*, object::*, utils::*};

use crate::{
	api::{
		locations::{file_path_with_object, object_with_file_paths, ExplorerItem},
		utils::library,
	},
	library::Library,
	location::{non_indexed, LocationError},
	object::media::thumbnail::get_thumb_key,
};

use std::path::PathBuf;

use prisma_client_rust::{or, WhereQuery};
use rspc::{alpha::AlphaRouter, ErrorCode};
use sd_prisma::prisma;
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{Ctx, R};

const MAX_TAKE: u8 = 100;

#[derive(Serialize, Type, Debug)]
struct SearchData<T> {
	cursor: Option<Vec<u8>>,
	items: Vec<T>,
}

pub fn mount() -> AlphaRouter<Ctx> {
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

			R.with2(library()).query(
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
				order_and_pagination: Option<
					OrderAndPagination<prisma::file_path::id::Type, FilePathOrder, FilePathCursor>,
				>,
				#[serde(default)]
				filter: FilePathFilterArgs,
				#[serde(default = "default_group_directories")]
				group_directories: bool,
			}

			fn default_group_directories() -> bool {
				true
			}

			R.with2(library()).query(
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
						query = query
							.order_by(prisma::file_path::is_dir::order(prisma::SortOrder::Desc));
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
									query.add_where(prisma::file_path::is_dir::not(Some(true)))
								}

								macro_rules! arm {
									($field:ident, $item:ident) => {{
										let item = $item;

										let data = item.data.clone();

										query.add_where(or![
											match item.order {
												SortOrder::Asc =>
													prisma::file_path::$field::gt(data),
												SortOrder::Desc =>
													prisma::file_path::$field::lt(data),
											},
											prisma_client_rust::and![
												prisma::file_path::$field::equals(Some(item.data)),
												match item.order {
													SortOrder::Asc => prisma::file_path::id::gt(id),
													SortOrder::Desc =>
														prisma::file_path::id::lt(id),
												}
											]
										]);

										query = query.order_by(prisma::file_path::$field::order(
											item.order.into(),
										));
									}};
								}

								match cursor.variant {
									FilePathCursorVariant::None => {
										query.add_where(prisma::file_path::id::gt(id));
									}
									FilePathCursorVariant::SizeInBytes(order) => {
										query = query.order_by(
											prisma::file_path::size_in_bytes_bytes::order(
												order.into(),
											),
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
													SortOrder::Asc => {
														prisma::file_path::object::is(vec![
															prisma::object::$field::gt(item.data),
														])
													}
													SortOrder::Desc => {
														prisma::file_path::object::is(vec![
															prisma::object::$field::lt(item.data),
														])
													}
												});

												query = query.order_by(
													prisma::file_path::object::order(vec![
														prisma::object::$field::order(
															item.order.into(),
														),
													]),
												);
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

								query = query
									.order_by(prisma::file_path::id::order(prisma::SortOrder::Asc));
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
							thumbnail_key: file_path.cas_id.as_ref().map(|i| get_thumb_key(i)),
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

			R.with2(library())
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
					Option<OrderAndPagination<prisma::object::id::Type, ObjectOrder, ObjectCursor>>,
				#[serde(default)]
				filter: ObjectFilterArgs,
			}

			R.with2(library()).query(
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
												SortOrder::Asc => prisma::object::$field::gt(data),
												SortOrder::Desc => prisma::object::$field::lt(data),
											},
											prisma_client_rust::and![
												prisma::object::$field::equals(Some(item.data)),
												match item.order {
													SortOrder::Asc => prisma::object::id::gt(id),
													SortOrder::Desc => prisma::object::id::lt(id),
												}
											]
										]);

										query = query.order_by(prisma::object::$field::order(
											item.order.into(),
										));
									}};
								}

								match cursor {
									ObjectCursor::None => {
										query.add_where(prisma::object::id::gt(id));
									}
									ObjectCursor::Kind(item) => arm!(kind, item),
									ObjectCursor::DateAccessed(item) => arm!(date_accessed, item),
								}

								query = query
									.order_by(prisma::object::pub_id::order(prisma::SortOrder::Asc))
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
							thumbnail_key: cas_id.map(|i| get_thumb_key(i)),
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

			R.with2(library())
				.query(|(_, library), Args { filter }| async move {
					let Library { db, .. } = library.as_ref();

					Ok(db.object().count(filter.into_params()).exec().await? as u32)
				})
		})
}
