use crate::{
	invalidate_query,
	location::{
		fetch_location,
		indexer::{indexer_job::indexer_job_location, rules::IndexerRuleCreateArgs},
		scan_location, LocationCreateArgs, LocationError, LocationUpdateArgs,
	},
	object::preview::THUMBNAIL_CACHE_DIR_NAME,
	prisma::{file_path, indexer_rule, indexer_rules_in_location, location, object, tag},
	LocationManager,
};

use rspc::{self, internal::MiddlewareBuilderLike, ErrorCode, Type};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

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
	Path(Box<file_path_with_object::Data>),
	Object(Box<object_with_file_paths::Data>),
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

			t(|_, args: LocationExplorerArgs, library| async move {
				let location = library
					.db
					.location()
					.find_unique(location::id::equals(args.location_id))
					.exec()
					.await?
					.ok_or_else(|| {
						rspc::Error::new(ErrorCode::NotFound, "Location not found".into())
					})?;

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

				Ok(ExplorerData {
					context: ExplorerContext::Location(location),
					items: file_paths
						.into_iter()
						.map(|mut file_path| {
							if let Some(object) = &mut file_path.object.as_mut() {
								// TODO: Use helper function to build this url as as the Rust file loading layer
								let thumb_path = library
									.config()
									.data_directory()
									.join(THUMBNAIL_CACHE_DIR_NAME)
									.join(&object.cas_id)
									.with_extension("webp");

								object.has_thumbnail = thumb_path.try_exists().unwrap();
							}
							ExplorerItem::Path(Box::new(file_path))
						})
						.collect(),
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
				library
					.db
					.file_path()
					.delete_many(vec![file_path::location_id::equals(location_id)])
					.exec()
					.await?;

				library
					.db
					.indexer_rules_in_location()
					.delete_many(vec![indexer_rules_in_location::location_id::equals(
						location_id,
					)])
					.exec()
					.await?;

				library
					.db
					.location()
					.delete(location::id::equals(location_id))
					.exec()
					.await?;

				invalidate_query!(library, "locations.list");
				if let Err(e) = LocationManager::global().remove(location_id).await {
					error!("Failed to remove location from manager: {e:#?}");
				}

				info!("Location {} deleted", location_id);

				Ok(())
			})
		})
		.library_mutation("fullRescan", |t| {
			t(|_, location_id: i32, library| async move {
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
