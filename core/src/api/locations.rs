use crate::{
	location::{
		delete_location, fetch_location,
		indexer::{indexer_job::indexer_job_location, rules::IndexerRuleCreateArgs},
		relink_location, scan_location, LocationCreateArgs, LocationError, LocationUpdateArgs,
	},
	prisma::{file_path, indexer_rule, indexer_rules_in_location, location, object, tag},
};

use std::path::PathBuf;

use rspc::{self, internal::MiddlewareBuilderLike, ErrorCode, Type};
use serde::{Deserialize, Serialize};

use super::{utils::LibraryRequest, Ctx, RouterBuilder};

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
		has_thumbnail: bool,
		item: Box<file_path_with_object::Data>,
	},
	Object {
		has_thumbnail: bool,
		item: Box<object_with_file_paths::Data>,
	},
}

#[derive(Serialize, Deserialize, Type, Debug)]
pub struct ExplorerData {
	pub context: ExplorerContext,
	pub items: Vec<ExplorerItem>,
}

file_path::include!(file_path_with_object { object });
object::include!(object_with_file_paths { file_paths });

// TODO(@Oscar): This return type sucks. Add an upstream rspc solution.
pub(crate) fn mount() -> rspc::RouterBuilder<
	Ctx,
	(),
	impl MiddlewareBuilderLike<Ctx, LayerContext = Ctx> + Send + 'static,
> {
	<RouterBuilder>::new()
		.library_query("list", |t| {
			t(|_, _: (), library| async move {
				Ok(library
					.db
					.location()
					.find_many(vec![])
					.include(location::include!({ node }))
					.exec()
					.await?)
			})
		})
		.library_query("getById", |t| {
			t(|_, location_id: i32, library| async move {
				Ok(library
					.db
					.location()
					.find_unique(location::id::equals(location_id))
					.exec()
					.await?)
			})
		})
		.library_query("getExplorerData", |t| {
			#[derive(Clone, Serialize, Deserialize, Type, Debug)]
			pub struct LocationExplorerArgs {
				pub location_id: i32,
				pub path: String,
				pub limit: i32,
				pub cursor: Option<String>,
			}

			t(|_, mut args: LocationExplorerArgs, library| async move {
				let location = library
					.db
					.location()
					.find_unique(location::id::equals(args.location_id))
					.exec()
					.await?
					.ok_or_else(|| {
						rspc::Error::new(ErrorCode::NotFound, "Location not found".into())
					})?;

				if !args.path.ends_with('/') {
					args.path += "/";
				}

				let directory = library
					.db
					.file_path()
					.find_first(vec![
						file_path::location_id::equals(location.id),
						file_path::materialized_path::equals(args.path),
						file_path::is_dir::equals(true),
					])
					.exec()
					.await?
					.ok_or_else(|| {
						rspc::Error::new(ErrorCode::NotFound, "Directory not found".into())
					})?;

				let file_paths = library
					.db
					.file_path()
					.find_many(vec![
						file_path::location_id::equals(location.id),
						file_path::parent_id::equals(Some(directory.id)),
					])
					.include(file_path_with_object::include())
					.exec()
					.await?;

				// library
				// 	.queue_job(Job::new(
				// 		ThumbnailJobInit {
				// 			location_id: location.id,
				// 			// recursive: false, // TODO: do this
				// 			root_path: PathBuf::from(&directory.materialized_path),
				// 			background: true,
				// 		},
				// 		ThumbnailJob {},
				// 	))
				// 	.await;

				let mut items = Vec::with_capacity(file_paths.len());

				for file_path in file_paths {
					let has_thumbnail = match &file_path.cas_id {
						None => false,
						Some(cas_id) => library
							.thumbnail_exists(cas_id)
							.await
							.map_err(LocationError::IOError)?,
					};

					items.push(ExplorerItem::Path {
						has_thumbnail,
						item: Box::new(file_path),
					});
				}

				Ok(ExplorerData {
					context: ExplorerContext::Location(location),
					items,
				})
			})
		})
		.library_mutation("create", |t| {
			t(|_, args: LocationCreateArgs, library| async move {
				let location = args.create(&library).await?;
				scan_location(&library, location).await?;
				Ok(())
			})
		})
		.library_mutation("update", |t| {
			t(|_, args: LocationUpdateArgs, library| async move {
				args.update(&library).await.map_err(Into::into)
			})
		})
		.library_mutation("delete", |t| {
			t(|_, location_id: i32, library| async move {
				delete_location(&library, location_id)
					.await
					.map_err(Into::into)
			})
		})
		.library_mutation("relink", |t| {
			t(|_, location_path: PathBuf, library| async move {
				relink_location(&library, location_path)
					.await
					.map_err(Into::into)
			})
		})
		.library_mutation("addLibrary", |t| {
			t(|_, args: LocationCreateArgs, library| async move {
				let location = args.add_library(&library).await?;
				scan_location(&library, location).await?;
				Ok(())
			})
		})
		.library_mutation("fullRescan", |t| {
			t(|_, location_id: i32, library| async move {
				// remove existing paths
				library
					.db
					.file_path()
					.delete_many(vec![file_path::location_id::equals(location_id)])
					.exec()
					.await?;
				// rescan location
				scan_location(
					&library,
					fetch_location(&library, location_id)
						.include(indexer_job_location::include())
						.exec()
						.await?
						.ok_or(LocationError::IdNotFound(location_id))?,
				)
				.await
				.map_err(Into::into)
			})
		})
		.library_mutation("quickRescan", |t| {
			t(|_, _: (), _| async move {
				#[allow(unreachable_code)]
				Ok(todo!())
			})
		})
		.merge("indexer_rules.", mount_indexer_rule_routes())
}

fn mount_indexer_rule_routes() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_mutation("create", |t| {
			t(|_, args: IndexerRuleCreateArgs, library| async move {
				args.create(&library).await.map_err(Into::into)
			})
		})
		.library_mutation("delete", |t| {
			t(|_, indexer_rule_id: i32, library| async move {
				library
					.db
					.indexer_rules_in_location()
					.delete_many(vec![indexer_rules_in_location::indexer_rule_id::equals(
						indexer_rule_id,
					)])
					.exec()
					.await?;

				library
					.db
					.indexer_rule()
					.delete(indexer_rule::id::equals(indexer_rule_id))
					.exec()
					.await?;

				Ok(())
			})
		})
		.library_query("get", |t| {
			t(|_, indexer_rule_id: i32, library| async move {
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
		.library_query("list", |t| {
			t(|_, _: (), library| async move {
				library
					.db
					.indexer_rule()
					.find_many(vec![])
					.exec()
					.await
					.map_err(Into::into)
			})
		})
}
