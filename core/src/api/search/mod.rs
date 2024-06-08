use crate::{
	api::{locations::ExplorerItem, utils::library},
	library::Library,
	location::{non_indexed, LocationError},
	util::{unsafe_streamed_query, BatchedStream},
};

use prisma_client_rust::Operator;
use sd_core_heavy_lifting::media_processor::ThumbKey;
use sd_core_prisma_helpers::{file_path_for_frontend, object_with_file_paths, CasId};
use sd_prisma::prisma::{self, PrismaClient};

use std::path::PathBuf;

use async_stream::stream;
use futures::StreamExt;
use itertools::Either;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;

pub mod exif_data;
pub mod file_path;
pub mod object;
pub mod saved;
mod utils;

pub use self::{file_path::*, object::*, utils::*};

use super::{Ctx, R};

const MAX_TAKE: u8 = 100;

#[derive(Serialize, Type, Debug)]
struct SearchData<T> {
	cursor: Option<Vec<u8>>,
	items: Vec<T>,
}

#[derive(Serialize, Deserialize, Type, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum SearchFilterArgs {
	FilePath(FilePathFilterArgs),
	Object(ObjectFilterArgs),
}

impl SearchFilterArgs {
	async fn into_params(
		self,
		db: &PrismaClient,
		file_path: &mut Vec<prisma::file_path::WhereParam>,
		object: &mut Vec<prisma::object::WhereParam>,
	) -> Result<(), rspc::Error> {
		match self {
			Self::FilePath(v) => file_path.extend(v.into_params(db).await?),
			Self::Object(v) => object.extend(v.into_params()),
		};
		Ok(())
	}
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
			#[derive(Serialize, Type, Debug)]
			struct EphemeralPathsResultItem {
				pub entries: Vec<ExplorerItem>,
				pub errors: Vec<rspc::Error>,
			}

			R.with2(library()).subscription(
				|(node, library),
				 EphemeralPathSearchArgs {
				     path,
				     with_hidden_files,
				     order,
				 }| async move {
					let paths =
						non_indexed::walk(path, with_hidden_files, node, library, |entries| {
							macro_rules! order_match {
								($order:ident, [$(($variant:ident, |$i:ident| $func:expr)),+]) => {{
									match $order {
										$(EphemeralPathOrder::$variant(order) => {
											entries.sort_unstable_by(|path1, path2| {
												let func = |$i: &non_indexed::Entry| $func;

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
						})
						.await?;

					let mut stream = BatchedStream::new(paths);
					Ok(unsafe_streamed_query(stream! {
						while let Some(result) = stream.next().await {
							// We optimize for the case of no errors because it should be way more common.
							let mut entries = Vec::with_capacity(result.len());
							let mut errors = Vec::with_capacity(0);

							for item in result {
								match item {
									Ok(item) => entries.push(item),
									Err(e) => match e {
										Either::Left(e) => errors.push(e),
										Either::Right(e) => errors.push(e.into()),
									},
								}
							}

							yield EphemeralPathsResultItem {
								entries,
								errors,
							};
						}
					}))
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
				order_and_pagination: Option<file_path::OrderAndPagination>,
				#[serde(default)]
				filters: Vec<SearchFilterArgs>,
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
				     filters,
				     group_directories,
				 }| async move {
					let Library { db, .. } = library.as_ref();

					let params = {
						let (mut fp, obj) = merge_filters(filters, db).await?;

						if !obj.is_empty() {
							fp.push(prisma::file_path::object::is(obj));
						}

						fp
					};

					let mut query = db.file_path().find_many(andify(params));

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
						order_and_pagination.apply(&mut query, group_directories)
					}

					let file_paths = query
						.include(file_path_for_frontend::include())
						.exec()
						.await?;

					let mut items = Vec::with_capacity(file_paths.len());

					for file_path in file_paths {
						let has_created_thumbnail =
							if let Some(cas_id) = file_path.cas_id.as_ref().map(CasId::from) {
								library
									.thumbnail_exists(&node, &cas_id)
									.await
									.map_err(LocationError::from)?
							} else {
								false
							};

						items.push(ExplorerItem::Path {
							thumbnail: file_path
								.cas_id
								.as_ref()
								.map(CasId::from)
								.map(CasId::into_owned)
								.map(|cas_id| ThumbKey::new_indexed(cas_id, library.id)),
							has_created_thumbnail,
							item: Box::new(file_path),
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
				#[specta(default)]
				filters: Vec<SearchFilterArgs>,
			}

			R.with2(library())
				.query(|(_, library), Args { filters }| async move {
					let Library { db, .. } = library.as_ref();

					Ok(db
						.file_path()
						.count({
							let (mut fp, obj) = merge_filters(filters, db).await?;

							if !obj.is_empty() {
								fp.push(prisma::file_path::object::is(obj));
							}

							fp
						})
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
				order_and_pagination: Option<object::OrderAndPagination>,
				#[serde(default)]
				filters: Vec<SearchFilterArgs>,
			}

			R.with2(library()).query(
				|(node, library),
				 ObjectSearchArgs {
				     take,
				     order_and_pagination,
				     filters,
				 }| async move {
					let Library { db, .. } = library.as_ref();

					let take = take.max(MAX_TAKE);

					let mut query = db
						.object()
						.find_many({
							let (fp, mut obj) = merge_filters(filters, db).await?;

							if !fp.is_empty() {
								obj.push(prisma::object::file_paths::some(fp));
							}

							andify(obj)
						})
						.take(take as i64);

					if let Some(order_and_pagination) = order_and_pagination {
						order_and_pagination.apply(&mut query);
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
							.find_map(|c| c)
							.map(CasId::from)
							.map(|cas_id| cas_id.to_owned());

						let has_created_thumbnail = if let Some(cas_id) = &cas_id {
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
							thumbnail: cas_id
								.map(|cas_id| ThumbKey::new_indexed(cas_id, library.id)),
							item: object,
							has_created_thumbnail,
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
				filters: Vec<SearchFilterArgs>,
			}

			R.with2(library())
				.query(|(_, library), Args { filters }| async move {
					let Library { db, .. } = library.as_ref();

					Ok(db
						.object()
						.count({
							let (fp, mut obj) = merge_filters(filters, db).await?;

							if !fp.is_empty() {
								obj.push(prisma::object::file_paths::some(fp));
							}

							obj
						})
						.exec()
						.await? as u32)
				})
		})
		.merge("saved.", saved::mount())
}

async fn merge_filters(
	filters: Vec<SearchFilterArgs>,
	db: &PrismaClient,
) -> Result<
	(
		Vec<prisma::file_path::WhereParam>,
		Vec<prisma::object::WhereParam>,
	),
	rspc::Error,
> {
	let mut obj = vec![];
	let mut fp = vec![];

	for filter in filters {
		filter.into_params(db, &mut fp, &mut obj).await?;
	}

	Ok((fp, obj))
}

/// PCR 0.6.x's AND does { AND: [{ ...}] } instead of { AND: [{ ... }, { ... }, { ... }] },
/// this works around it.
fn andify<T: From<Operator<T>>>(params: Vec<T>) -> Vec<T> {
	params.into_iter().fold(vec![], |mut params, param| {
		params.push(param);

		vec![prisma_client_rust::operator::and(params)]
	})
}
