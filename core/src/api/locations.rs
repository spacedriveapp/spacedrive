use crate::{
	library::Library,
	location::{
		delete_location, file_path_helper::IsolatedFilePathData, find_location,
		indexer::rules::IndexerRuleCreateArgs, light_scan_location, location_with_indexer_rules,
		relink_location, scan_location, LocationCreateArgs, LocationError, LocationUpdateArgs,
	},
	prisma::{file_path, indexer_rule, indexer_rules_in_location, location, object, tag},
	util::db::chain_optional_iter,
};

use std::{collections::BTreeSet, path::PathBuf};

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
				pub path: Option<String>,
				pub limit: i32,
				pub cursor: Option<String>,
				pub kind: Option<Vec<i32>>,
			}

			R.with2(library())
				.query(|(_, library), args: LocationExplorerArgs| async move {
					let Library { db, .. } = &library;

					let location = find_location(&library, args.location_id)
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(args.location_id))?;

					let directory_materialized_path_str = if let Some(path) = args.path {
						let (materialized_path, maybe_name, _maybe_extension) =
							IsolatedFilePathData::separate_path_name_and_extension_from_str(&path);
						let parent_dir = db
							.file_path()
							.find_first(chain_optional_iter(
								[
									file_path::location_id::equals(location.id),
									file_path::materialized_path::equals(
										materialized_path.to_string(),
									),
									file_path::is_dir::equals(true),
								],
								[maybe_name.map(str::to_string).map(file_path::name::equals)],
							))
							.select(file_path::select!({ materialized_path name }))
							.exec()
							.await?
							.ok_or_else(|| {
								rspc::Error::new(ErrorCode::NotFound, "Directory not found".into())
							})?;

						Some(format!(
							"{}/{}/",
							parent_dir.materialized_path, parent_dir.name
						))
					} else {
						None
					};

					let expected_kinds = args
						.kind
						.map(|kinds| kinds.into_iter().collect::<BTreeSet<_>>())
						.unwrap_or_default();

					let mut file_paths = db
						.file_path()
						.find_many(chain_optional_iter(
							[file_path::location_id::equals(location.id)],
							[directory_materialized_path_str
								.map(file_path::materialized_path::equals)],
						))
						.include(file_path_with_object::include())
						.exec()
						.await?;

					if !expected_kinds.is_empty() {
						file_paths = file_paths
							.into_iter()
							.filter(|file_path| {
								file_path
									.object
									.map(|ref object| expected_kinds.contains(&object.kind))
									.unwrap_or(false)
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
					})
				})
		})
		.procedure("create", {
			R.with2(library())
				.mutation(|(_, library), args: LocationCreateArgs| async move {
					let location = args.create(&library).await?;
					scan_location(&library, location).await?;
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
					delete_location(&library, location_id)
						.await
						.map_err(Into::into)
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
					let location = args.add_library(&library).await?;
					scan_location(&library, location).await?;
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
				let location_manager = ctx.library_manager.node_context.location_manager.clone();

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
					args.create(&library).await.map_err(Into::into)
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
