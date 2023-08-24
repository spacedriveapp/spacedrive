use crate::{
	invalidate_query,
	location::{
		delete_location, find_location, indexer::rules::IndexerRuleCreateArgs, light_scan_location,
		location_with_indexer_rules, non_indexed::NonIndexedPathItem, relink_location,
		scan_location, scan_location_sub_path, LocationCreateArgs, LocationError,
		LocationUpdateArgs,
	},
	prisma::{file_path, indexer_rule, indexer_rules_in_location, location, object, SortOrder},
	util::AbortOnDrop,
};

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use rspc::{self, alpha::AlphaRouter, ErrorCode};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{utils::library, Ctx, R};

#[derive(Serialize, Type, Debug)]
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
	Location {
		has_local_thumbnail: bool,
		thumbnail_key: Option<Vec<String>>,
		item: location::Data,
	},
	NonIndexedPath {
		has_local_thumbnail: bool,
		thumbnail_key: Option<Vec<String>>,
		item: NonIndexedPathItem,
	},
}

impl ExplorerItem {
	pub fn name(&self) -> &str {
		match self {
			ExplorerItem::Path {
				item: file_path_with_object::Data { name, .. },
				..
			}
			| ExplorerItem::Location {
				item: location::Data { name, .. },
				..
			} => name.as_deref().unwrap_or(""),
			ExplorerItem::NonIndexedPath { item, .. } => item.name.as_str(),
			_ => "",
		}
	}

	pub fn size_in_bytes(&self) -> u64 {
		match self {
			ExplorerItem::Path {
				item: file_path_with_object::Data {
					size_in_bytes_bytes,
					..
				},
				..
			} => size_in_bytes_bytes
				.as_ref()
				.map(|size| {
					u64::from_be_bytes([
						size[0], size[1], size[2], size[3], size[4], size[5], size[6], size[7],
					])
				})
				.unwrap_or(0),

			ExplorerItem::NonIndexedPath {
				item: NonIndexedPathItem {
					size_in_bytes_bytes,
					..
				},
				..
			} => u64::from_be_bytes([
				size_in_bytes_bytes[0],
				size_in_bytes_bytes[1],
				size_in_bytes_bytes[2],
				size_in_bytes_bytes[3],
				size_in_bytes_bytes[4],
				size_in_bytes_bytes[5],
				size_in_bytes_bytes[6],
				size_in_bytes_bytes[7],
			]),
			_ => 0,
		}
	}

	pub fn date_created(&self) -> DateTime<Utc> {
		match self {
			ExplorerItem::Path {
				item: file_path_with_object::Data { date_created, .. },
				..
			}
			| ExplorerItem::Object {
				item: object_with_file_paths::Data { date_created, .. },
				..
			}
			| ExplorerItem::Location {
				item: location::Data { date_created, .. },
				..
			} => date_created.map(Into::into).unwrap_or_default(),

			ExplorerItem::NonIndexedPath { item, .. } => item.date_created,
		}
	}

	pub fn date_modified(&self) -> DateTime<Utc> {
		match self {
			ExplorerItem::Path { item, .. } => {
				item.date_modified.map(Into::into).unwrap_or_default()
			}
			ExplorerItem::NonIndexedPath { item, .. } => item.date_modified,
			_ => Default::default(),
		}
	}
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
					.order_by(location::date_created::order(SortOrder::Desc))
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
				.mutation(|(node, library), args: LocationCreateArgs| async move {
					if let Some(location) = args.create(&node, &library).await? {
						scan_location(&node, &library, location).await?;
						invalidate_query!(library, "locations.list");
					}

					Ok(())
				})
		})
		.procedure("update", {
			R.with2(library())
				.mutation(|(_, library), args: LocationUpdateArgs| async move {
					let ret = args.update(&library).await.map_err(Into::into);
					invalidate_query!(library, "locations.list");
					ret
				})
		})
		.procedure("delete", {
			R.with2(library()).mutation(
				|(node, library), location_id: location::id::Type| async move {
					delete_location(&node, &library, location_id).await?;
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
				.mutation(|(node, library), args: LocationCreateArgs| async move {
					if let Some(location) = args.add_library(&node, &library).await? {
						scan_location(&node, &library, location).await?;
						invalidate_query!(library, "locations.list");
					}
					Ok(())
				})
		})
		.procedure("fullRescan", {
			#[derive(Type, Deserialize)]
			pub struct FullRescanArgs {
				pub location_id: location::id::Type,
				pub reidentify_objects: bool,
			}

			R.with2(library()).mutation(
				|(node, library),
				 FullRescanArgs {
				     location_id,
				     reidentify_objects,
				 }| async move {
					if reidentify_objects {
						library
							.db
							.file_path()
							.update_many(
								vec![
									file_path::location_id::equals(Some(location_id)),
									file_path::object_id::not(None),
								],
								vec![file_path::object::disconnect()],
							)
							.exec()
							.await?;

						library.orphan_remover.invoke().await;
					}

					// rescan location
					scan_location(
						&node,
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
		.procedure("subPathRescan", {
			#[derive(Clone, Serialize, Deserialize, Type, Debug)]
			pub struct RescanArgs {
				pub location_id: location::id::Type,
				pub sub_path: String,
			}

			R.with2(library()).mutation(
				|(node, library),
				 RescanArgs {
				     location_id,
				     sub_path,
				 }: RescanArgs| async move {
					scan_location_sub_path(
						&node,
						&library,
						find_location(&library, location_id)
							.include(location_with_indexer_rules::include())
							.exec()
							.await?
							.ok_or(LocationError::IdNotFound(location_id))?,
						sub_path,
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
				.subscription(|(node, library), args: LightScanArgs| async move {
					let location = find_location(&library, args.location_id)
						.include(location_with_indexer_rules::include())
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(args.location_id))?;

					let handle =
						tokio::spawn(light_scan_location(node, library, location, args.sub_path));

					Ok(AbortOnDrop(handle))
				})
		})
		.procedure(
			"online",
			R.subscription(|node, _: ()| async move {
				let mut rx = node.locations.online_rx();

				async_stream::stream! {
					let online = node.locations.get_online().await;

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
	// .procedure("createDirectory", {
	// 	#[derive(Type, Deserialize)]
	// 	struct CreateDirectoryArgs {
	// 		location_id: location::id::Type,
	// 		subpath: String,
	// 	}
	// 	R.with2(library())
	// 		.query(|(_, library), args: CreateDirectoryArgs| async move {
	// 			let location = find_location(&library, args.location_id)
	// 				.exec()
	// 				.await?
	// 				.ok_or(LocationError::IdNotFound(args.location_id))?;

	// 			let mut path = Path::new(&location.path.unwrap_or_default());
	// 			path.push(args.subpath);

	// 			Ok(())
	// 		})
	// })
}
