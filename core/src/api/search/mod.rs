use std::{collections::HashMap, path::PathBuf};

use crate::{
	api::{locations::ExplorerItem, utils::library},
	library::Library,
	location::LocationError,
	object::{
		cas::generate_cas_id,
		media::old_thumbnail::{
			get_ephemeral_thumb_key, get_indexed_thumb_key, BatchToProcess, GenerateThumbnailArgs,
		},
	},
	util::{unsafe_streamed_query, BatchedStream},
};

use opendal::{services::Fs, Operator};

use sd_cache::{CacheNode, Model, Normalise, Reference};
use sd_core_indexer_rules::seed::{no_hidden, no_os_protected};
use sd_core_indexer_rules::IndexerRule;
use sd_core_prisma_helpers::{file_path_with_object, object_with_file_paths};
use sd_file_ext::kind::ObjectKind;
use sd_prisma::prisma::{self, location, PrismaClient};
use sd_utils::chain_optional_iter;

use async_stream::stream;
use futures::StreamExt;
use rspc::{alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::{error, warn};

pub mod file_path;
pub mod media_data;
pub mod object;
pub mod saved;
mod utils;

pub use self::{file_path::*, object::*, utils::*};

use super::{Ctx, R};

const MAX_TAKE: u8 = 100;

#[derive(Serialize, Type, Debug)]
struct SearchData<T: Model> {
	cursor: Option<Vec<u8>>,
	items: Vec<Reference<T>>,
	nodes: Vec<CacheNode>,
}

impl<T: Model> Model for SearchData<T> {
	fn name() -> &'static str {
		T::name()
	}
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
		Ok(match self {
			Self::FilePath(v) => file_path.extend(v.into_params(db).await?),
			Self::Object(v) => object.extend(v.into_params()),
		})
	}
}

pub fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("ephemeralPaths", {
			#[derive(Deserialize, Type, Debug, PartialEq, Eq)]
			#[serde(rename_all = "camelCase")]
			enum PathFrom {
				Path,
				// TODO: FTP + S3 + GDrive
			}

			#[derive(Deserialize, Type, Debug)]
			#[serde(rename_all = "camelCase")]
			struct EphemeralPathSearchArgs {
				from: PathFrom,
				path: String,
				with_hidden_files: bool,
			}

			#[derive(Serialize, Type, Debug)]
			struct EphemeralPathsResultItem {
				pub entries: Vec<Reference<ExplorerItem>>,
				pub errors: Vec<String>,
				pub nodes: Vec<CacheNode>,
			}

			R.with2(library()).subscription(
				|(node, library),
				 EphemeralPathSearchArgs {
				     from,
				     mut path,
				     with_hidden_files,
				 }| async move {
					let service = match from {
						PathFrom::Path => {
							let mut fs = Fs::default();
							fs.root("/");
							Operator::new(fs)
								.map_err(|err| {
									rspc::Error::new(
										ErrorCode::InternalServerError,
										err.to_string(),
									)
								})?
								.finish()
						}
					};

					let rules = chain_optional_iter(
						[],
						[(!with_hidden_files).then(|| IndexerRule::from(no_hidden()))],
					);

					// OpenDAL is specific about paths (and the rest of Spacedrive is not)
					if !path.ends_with('/') {
						path.push('/');
					}

					let stream =
						sd_indexer::ephemeral(service, rules, &path)
							.await
							.map_err(|err| {
								rspc::Error::new(ErrorCode::InternalServerError, err.to_string())
							})?;

					let mut stream = BatchedStream::new(stream);
					Ok(unsafe_streamed_query(stream! {
						let mut to_generate = vec![];

						while let Some(result) = stream.next().await {
							// We optimize for the case of no errors because it should be way more common.
							let mut entries = Vec::with_capacity(result.len());
							let mut errors = Vec::with_capacity(0);

							// For this batch we check if any directories are actually locations, so the UI can link directly to them
							let locations = library
								.db
								.location()
								.find_many(vec![location::path::in_vec(
									result.iter().filter_map(|e| match e {
										Ok(e) if ObjectKind::from_i32(e.kind) == ObjectKind::Folder => Some(e.path.clone()),
										_ => None
									}).collect::<Vec<_>>()
								)])
								.exec()
								.await
								.and_then(|l| {
									Ok(l.into_iter()
										.filter_map(|item| item.path.clone().map(|l| (l, item)))
										.collect::<HashMap<_, _>>())
								})
								.map_err(|err| error!("Looking up locations failed: {err:?}"))
								.unwrap_or_default();

							for item in result {
								match item {
									Ok(item) => {
										let kind = ObjectKind::from_i32(item.kind);
										let should_generate_thumbnail = {
											#[cfg(feature = "ffmpeg")]
											{
												matches!(
													kind,
													ObjectKind::Image | ObjectKind::Video | ObjectKind::Document
												)
											}

											#[cfg(not(feature = "ffmpeg"))]
											{
												matches!(kind, ObjectKind::Image | ObjectKind::Document)
											}
										};

										// TODO: This requires all paths to be loaded before thumbnailing starts.
										// TODO: This copies the existing functionality but will not fly with Cloud locations (as loading paths will be *way* slower)
										// TODO: https://linear.app/spacedriveapp/issue/ENG-1719/cloud-thumbnailer
										let thumbnail = if should_generate_thumbnail {
											if from == PathFrom::Path {
												let size = u64::from_be_bytes((&*item.size_in_bytes_bytes).try_into().expect("Invalid size"));
												if let Ok(cas_id) = generate_cas_id(&item.path, size).await.map_err(|err| error!("Error generating cas id for '{:?}': {err:?}", item.path)) {
													if ObjectKind::from_i32(item.kind) == ObjectKind::Document {
														to_generate.push(GenerateThumbnailArgs::new(
															item.extension.clone(),
															cas_id.clone(),
															PathBuf::from(&item.path),
														));
													} else {
														to_generate.push(GenerateThumbnailArgs::new(
															item.extension.clone(),
															cas_id.clone(),
															PathBuf::from(&item.path),
														));
													}

													Some(get_ephemeral_thumb_key(&cas_id))
												} else {
													None
												}
											} else {
												warn!("Thumbnailer not supported for cloud locations");
												None
											}
										} else {
											None
										};

										entries.push(if let Some(item) = locations.get(&item.path) {
											ExplorerItem::Location {
												item: item.clone(),
											}
										} else {
											ExplorerItem::NonIndexedPath {
												thumbnail,
												item,
											}
										});
									},
									Err(e) => errors.push(e.to_string()),
								}
							}

							let (nodes, entries) = entries.normalise(|item: &ExplorerItem| item.id());

							yield EphemeralPathsResultItem {
								entries,
								errors,
								nodes,
							};
						}

						if to_generate.len() > 0 {
							node.thumbnailer
								.new_ephemeral_thumbnails_batch(BatchToProcess::new(
									to_generate,
									false,
									false,
								))
								.await;
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

					let mut query = db.file_path().find_many(params);

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
							thumbnail: file_path
								.cas_id
								.as_ref()
								.filter(|_| thumbnail_exists_locally)
								.map(|i| get_indexed_thumb_key(i, library.id)),
							item: file_path,
						})
					}

					let (nodes, items) = items.normalise(|item| item.id());

					Ok(SearchData {
						items,
						cursor: None,
						nodes,
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

							obj
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
							thumbnail: cas_id
								.filter(|_| thumbnail_exists_locally)
								.map(|cas_id| get_indexed_thumb_key(cas_id, library.id)),
							item: object,
						});
					}

					let (nodes, items) = items.normalise(|item| item.id());

					Ok(SearchData {
						nodes,
						items,
						cursor,
					})
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
