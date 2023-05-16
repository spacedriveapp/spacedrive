use crate::{
	invalidate_query,
	library::Library,
	location::{
		delete_location, find_location, indexer::rules::IndexerRuleCreateArgs, light_scan_location,
		location_with_indexer_rules, relink_location, scan_location, LocationCreateArgs,
		LocationError, LocationUpdateArgs,
	},
	prisma::{file_path, indexer_rule, indexer_rules_in_location, location, object, tag},
	util::db::chain_optional_iter,
};

use std::{
	collections::BTreeSet,
	path::{PathBuf, MAIN_SEPARATOR, MAIN_SEPARATOR_STR},
};

use rspc::{self, alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{utils::library, Ctx, R};

#[derive(Serialize, Deserialize, Type, Debug)]
#[serde(tag = "type")]
pub enum ExplorerContext {
	Location(location::Data),
	Tag(tag::Data),
	// Space(object_in_space::Data),
}

#[derive(Serialize, Deserialize, Type, Debug)]
#[serde(tag = "type")]
pub enum ExplorerItem {
	Path {
		// has_thumbnail is determined by the local existence of a thumbnail
		has_thumbnail: bool,
		item: file_path_with_object::Data,
	},
	Object {
		has_thumbnail: bool,
		item: object_with_file_paths::Data,
	},
}

#[derive(Serialize, Deserialize, Type, Debug)]
pub struct ExplorerData {
	pub context: ExplorerContext,
	pub items: Vec<ExplorerItem>,
	pub cursor: Option<Vec<u8>>,
}

file_path::include!(file_path_with_object { object });
object::include!(object_with_file_paths { file_paths });

pub(crate) fn mount() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("list", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				Ok(library
					.db
					.location()
					.find_many(vec![])
					.include(location::include!({ node }))
					.exec()
					.await?)
			})
		})
		.procedure("getById", {
			R.with2(library())
				.query(|(_, library), location_id: i32| async move {
					Ok(library
						.db
						.location()
						.find_unique(location::id::equals(location_id))
						.include(location_with_indexer_rules::include())
						.exec()
						.await?)
				})
		})
		.procedure("getExplorerData", {
			#[derive(Clone, Serialize, Deserialize, Type, Debug)]
			pub struct LocationExplorerArgs {
				pub location_id: i32,
				#[specta(optional)]
				pub path: Option<String>,
				#[specta(optional)]
				pub limit: Option<i32>,
				#[specta(optional)]
				pub cursor: Option<Vec<u8>>,
				#[specta(optional)]
				pub kind: Option<Vec<i32>>,
			}

			R.with2(library())
				.query(|(_, library), args: LocationExplorerArgs| async move {
					let Library { db, .. } = &library;

					dbg!(&args);

					let location = find_location(&library, args.location_id)
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(args.location_id))?;

					let directory_id = if let Some(mut path) = args.path {
						if !path.ends_with(MAIN_SEPARATOR) {
							path += MAIN_SEPARATOR_STR;
						}

						Some(
							db.file_path()
								.find_first(vec![
									file_path::location_id::equals(location.id),
									file_path::materialized_path::equals(path),
									file_path::is_dir::equals(true),
								])
								.select(file_path::select!({ pub_id }))
								.exec()
								.await?
								.ok_or_else(|| {
									rspc::Error::new(
										ErrorCode::NotFound,
										"Directory not found".into(),
									)
								})?
								.pub_id,
						)
					} else {
						None
					};

					let expected_kinds = args
						.kind
						.map(|kinds| kinds.into_iter().collect::<BTreeSet<_>>())
						.unwrap_or_default();

					let (mut file_paths, cursor) = {
						let limit = args.limit.unwrap_or(100);

						let mut query = db
							.file_path()
							.find_many(chain_optional_iter(
								[file_path::location_id::equals(location.id)],
								[directory_id.map(Some).map(file_path::parent_id::equals)],
							))
							.take((limit + 1) as i64);

						if let Some(cursor) = args.cursor {
							query = query.cursor(file_path::pub_id::equals(cursor));
						}

						let mut results = query
							.include(file_path_with_object::include())
							.exec()
							.await?;

						let cursor = if results.len() as i32 > limit {
							results.pop().map(|r| r.pub_id)
						} else {
							None
						};

						(results, cursor)
					};

					if !expected_kinds.is_empty() {
						file_paths = file_paths
							.into_iter()
							.filter(|file_path| {
								if let Some(ref object) = file_path.object {
									expected_kinds.contains(&object.kind)
								} else {
									false
								}
							})
							.collect::<Vec<_>>();
					}

					let mut items = Vec::with_capacity(file_paths.len());
					for file_path in file_paths {
						let has_thumbnail = if let Some(cas_id) = &file_path.cas_id {
							library
								.thumbnail_exists(cas_id)
								.await
								.map_err(LocationError::IOError)?
						} else {
							false
						};

						items.push(ExplorerItem::Path {
							has_thumbnail,
							item: file_path,
						});
					}

					Ok(ExplorerData {
						context: ExplorerContext::Location(location),
						items,
						cursor,
					})
				})
		})
		.procedure("create", {
			R.with2(library())
				.mutation(|(_, library), args: LocationCreateArgs| async move {
					if let Some(location) = args.create(&library).await? {
						scan_location(&library, location).await?;
						invalidate_query!(library, "locations.list");
					}

					Ok(())
				})
		})
		.procedure("update", {
			R.with2(library())
				.mutation(|(_, library), args: LocationUpdateArgs| async move {
					args.update(&library).await.map_err(Into::into)
				})
		})
		.procedure("delete", {
			R.with2(library())
				.mutation(|(_, library), location_id: i32| async move {
					delete_location(&library, location_id).await?;
					invalidate_query!(library, "locations.list");
					Ok(())
				})
		})
		.procedure("relink", {
			R.with2(library())
				.mutation(|(_, library), location_path: PathBuf| async move {
					relink_location(&library, location_path)
						.await
						.map_err(Into::into)
				})
		})
		.procedure("addLibrary", {
			R.with2(library())
				.mutation(|(_, library), args: LocationCreateArgs| async move {
					if let Some(location) = args.add_library(&library).await? {
						scan_location(&library, location).await?;
						invalidate_query!(library, "locations.list");
					}
					Ok(())
				})
		})
		.procedure("fullRescan", {
			R.with2(library())
				.mutation(|(_, library), location_id: i32| async move {
					// rescan location
					scan_location(
						&library,
						find_location(&library, location_id)
							.include(location_with_indexer_rules::include())
							.exec()
							.await?
							.ok_or(LocationError::IdNotFound(location_id))?,
					)
					.await
					.map_err(Into::into)
				})
		})
		.procedure("quickRescan", {
			#[derive(Clone, Serialize, Deserialize, Type, Debug)]
			pub struct LightScanArgs {
				pub location_id: i32,
				pub sub_path: String,
			}

			R.with2(library())
				.mutation(|(_, library), args: LightScanArgs| async move {
					// light rescan location
					light_scan_location(
						&library,
						find_location(&library, args.location_id)
							.include(location_with_indexer_rules::include())
							.exec()
							.await?
							.ok_or(LocationError::IdNotFound(args.location_id))?,
						&args.sub_path,
					)
					.await
					.map_err(Into::into)
				})
		})
		.procedure(
			"online",
			R.subscription(|ctx, _: ()| async move {
				let location_manager = ctx.location_manager.clone();

				let mut rx = location_manager.online_rx();

				async_stream::stream! {
					let online = location_manager.get_online().await;

					yield online;

					while let Ok(locations) = rx.recv().await {
						yield locations;
					}
				}
			}),
		)
		.merge("indexer_rules.", mount_indexer_rule_routes())
}

fn mount_indexer_rule_routes() -> AlphaRouter<Ctx> {
	R.router()
		.procedure("create", {
			R.with2(library())
				.mutation(|(_, library), args: IndexerRuleCreateArgs| async move {
					if args.create(&library).await?.is_some() {
						invalidate_query!(library, "locations.indexer_rules.list");
					}

					Ok(())
				})
		})
		.procedure("delete", {
			R.with2(library())
				.mutation(|(_, library), indexer_rule_id: i32| async move {
					let indexer_rule_db = library.db.indexer_rule();

					if let Some(indexer_rule) = indexer_rule_db
						.to_owned()
						.find_unique(indexer_rule::id::equals(indexer_rule_id))
						.exec()
						.await?
					{
						if indexer_rule.default {
							return Err(rspc::Error::new(
								ErrorCode::Forbidden,
								format!("Indexer rule <id={indexer_rule_id}> can't be deleted"),
							));
						}
					} else {
						return Err(rspc::Error::new(
							ErrorCode::NotFound,
							format!("Indexer rule <id={indexer_rule_id}> not found"),
						));
					}

					library
						.db
						.indexer_rules_in_location()
						.delete_many(vec![indexer_rules_in_location::indexer_rule_id::equals(
							indexer_rule_id,
						)])
						.exec()
						.await?;

					indexer_rule_db
						.delete(indexer_rule::id::equals(indexer_rule_id))
						.exec()
						.await?;

					invalidate_query!(library, "locations.indexer_rules.list");

					Ok(())
				})
		})
		.procedure("get", {
			R.with2(library())
				.query(|(_, library), indexer_rule_id: i32| async move {
					library
						.db
						.indexer_rule()
						.find_unique(indexer_rule::id::equals(indexer_rule_id))
						.exec()
						.await?
						.ok_or_else(|| {
							rspc::Error::new(
								ErrorCode::NotFound,
								format!("Indexer rule <id={indexer_rule_id}> not found"),
							)
						})
				})
		})
		.procedure("list", {
			R.with2(library()).query(|(_, library), _: ()| async move {
				library
					.db
					.indexer_rule()
					.find_many(vec![])
					.exec()
					.await
					.map_err(Into::into)
			})
		})
		// list indexer rules for location, returning the indexer rule
		.procedure("listForLocation", {
			R.with2(library())
				.query(|(_, library), location_id: i32| async move {
					library
						.db
						.indexer_rule()
						.find_many(vec![indexer_rule::locations::some(vec![
							indexer_rules_in_location::location_id::equals(location_id),
						])])
						.exec()
						.await
						.map_err(Into::into)
				})
		})
}
