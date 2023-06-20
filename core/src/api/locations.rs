use crate::{
	invalidate_query,
	location::{
		delete_location, find_location, indexer::rules::IndexerRuleCreateArgs, light_scan_location,
		location_with_indexer_rules, relink_location, scan_location, LocationCreateArgs,
		LocationError, LocationUpdateArgs,
	},
	prisma::{file_path, indexer_rule, indexer_rules_in_location, location, object, tag},
	util::AbortOnDrop,
};

use std::path::PathBuf;

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
		// has_local_thumbnail is true only if there is local existence of a thumbnail
		has_local_thumbnail: bool,
		// thumbnail_key is present if there is a cas_id
		// it includes the shard hex formatted as (["f0", "cab34a76fbf3469f"])
		thumbnail_key: Option<Vec<String>>,
		item: file_path_with_object::Data,
	},
	Object {
		has_local_thumbnail: bool,
		thumbnail_key: Option<Vec<String>>,
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
		.procedure("get", {
			R.with2(library())
				.query(|(_, library), location_id: location::id::Type| async move {
					Ok(library
						.db
						.location()
						.find_unique(location::id::equals(location_id))
						.exec()
						.await?)
				})
		})
		.procedure("getWithRules", {
			R.with2(library())
				.query(|(_, library), location_id: location::id::Type| async move {
					Ok(library
						.db
						.location()
						.find_unique(location::id::equals(location_id))
						.include(location_with_indexer_rules::include())
						.exec()
						.await?)
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
			R.with2(library()).mutation(
				|(_, library), location_id: location::id::Type| async move {
					delete_location(&library, location_id).await?;
					invalidate_query!(library, "locations.list");
					Ok(())
				},
			)
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
			R.with2(library()).mutation(
				|(_, library), location_id: location::id::Type| async move {
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
				},
			)
		})
		.procedure("quickRescan", {
			#[derive(Clone, Serialize, Deserialize, Type, Debug)]
			pub struct LightScanArgs {
				pub location_id: location::id::Type,
				pub sub_path: String,
			}

			R.with2(library())
				.subscription(|(_, library), args: LightScanArgs| async move {
					let location = find_location(&library, args.location_id)
						.include(location_with_indexer_rules::include())
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(args.location_id))?;

					let handle =
						tokio::spawn(light_scan_location(library, location, args.sub_path));

					Ok(AbortOnDrop(handle))
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
						if indexer_rule.default.unwrap_or_default() {
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
				.query(|(_, library), location_id: location::id::Type| async move {
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
