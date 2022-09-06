use crate::{
	encode::THUMBNAIL_CACHE_DIR_NAME,
	invalidate_query,
	location::{
		fetch_location, indexer::indexer_rules::IndexerRuleCreateArgs, scan_location,
		with_indexer_rules, LocationCreateArgs, LocationError, LocationUpdateArgs,
	},
	prisma::{file, file_path, indexer_rule, indexer_rules_in_location, location, tag},
};

use rspc::{self, ErrorCode, Type};
use serde::{Deserialize, Serialize};
use tracing::info;

use super::{utils::LibraryRequest, RouterBuilder};

#[derive(Serialize, Deserialize, Type, Debug)]
pub struct ExplorerData {
	pub context: ExplorerContext,
	pub items: Vec<ExplorerItem>,
}

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
	Path(Box<file_path::Data>),
	Object(Box<file::Data>),
}

#[derive(Clone, Serialize, Deserialize, Type, Debug)]
pub struct LocationExplorerArgs {
	pub location_id: i32,
	pub path: String,
	pub limit: i32,
	pub cursor: Option<String>,
}

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_query("list", |_, _: (), library| async move {
			Ok(library
				.db
				.location()
				.find_many(vec![])
				.with(location::node::fetch())
				.exec()
				.await?)
		})
		.library_query("getById", |_, location_id: i32, library| async move {
			Ok(library
				.db
				.location()
				.find_unique(location::id::equals(location_id))
				.exec()
				.await?)
		})
		.library_query(
			"getExplorerData",
			|_, args: LocationExplorerArgs, library| async move {
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
					.with(file_path::file::fetch())
					.exec()
					.await?;

				Ok(ExplorerData {
					context: ExplorerContext::Location(location),
					items: file_paths
						.into_iter()
						.map(|mut file_path| {
							if let Some(file) = &mut file_path.file.as_mut().unwrap_or_else(
								|| /* Prisma relationship was not fetched */ unreachable!(),
							) {
								// TODO: Use helper function to build this url as as the Rust file loading layer
								let thumb_path = library
									.config()
									.data_directory()
									.join(THUMBNAIL_CACHE_DIR_NAME)
									.join(&file.cas_id)
									.with_extension("webp");

								file.has_thumbnail = thumb_path.exists();
							}
							ExplorerItem::Path(Box::new(file_path))
						})
						.collect(),
				})
			},
		)
		.library_mutation(
			"create",
			|_, args: LocationCreateArgs, library| async move {
				let location = args.create(&library).await?;
				scan_location(&library, &location).await?;

				Ok(location)
			},
		)
		.library_mutation(
			"update",
			|_, args: LocationUpdateArgs, library| async move {
				args.update(&library).await.map_err(Into::into)
			},
		)
		.library_mutation("delete", |_, location_id: i32, library| async move {
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

			info!("Location {} deleted", location_id);

			Ok(())
		})
		.library_mutation("fullRescan", |_, location_id: i32, library| async move {
			scan_location(
				&library,
				&fetch_location(&library, location_id)
					.with(with_indexer_rules(location_id))
					.exec()
					.await?
					.ok_or(LocationError::IdNotFound(location_id))?,
			)
			.await
			.map_err(Into::into)
		})
		.library_mutation("quickRescan", |_, _: (), _| async move {
			#[allow(unreachable_code)]
			Ok(todo!())
		})
		.merge("indexer_rules", mount_indexer_rule_routes())
}

fn mount_indexer_rule_routes() -> RouterBuilder {
	<RouterBuilder>::new()
		.library_mutation(
			"create",
			|_, args: IndexerRuleCreateArgs, library| async move {
				args.create(&library).await.map_err(Into::into)
			},
		)
		.library_mutation("delete", |_, indexer_rule_id: i32, library| async move {
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
		.library_query("get", |_, indexer_rule_id: i32, library| async move {
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
		.library_query("list", |_, _: (), library| async move {
			library
				.db
				.indexer_rule()
				.find_many(vec![])
				.exec()
				.await
				.map_err(Into::into)
		})
}
