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

use super::{LibraryArgs, RouterBuilder};

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
	Path(file_path::Data),
	Object(file::Data),
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
		.query("get", |ctx, arg: LibraryArgs<()>| async move {
			let (_, library) = arg.get_library(&ctx).await?;

			let locations = library
				.db
				.location()
				.find_many(vec![])
				.with(location::node::fetch())
				.exec()
				.await?;

			Ok(locations)
		})
		.query("getById", |ctx, arg: LibraryArgs<i32>| async move {
			let (location_id, library) = arg.get_library(&ctx).await?;

			Ok(library
				.db
				.location()
				.find_unique(location::id::equals(location_id))
				.exec()
				.await?)
		})
		.query(
			"getExplorerData",
			|ctx, arg: LibraryArgs<LocationExplorerArgs>| async move {
				let (args, library) = arg.get_library(&ctx).await?;

				let location = library
					.db
					.location()
					.find_unique(location::id::equals(args.location_id))
					.exec()
					.await?
					.unwrap();

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
							ExplorerItem::Path(file_path)
						})
						.collect(),
				})
			},
		)
		.mutation(
			"create",
			|ctx, arg: LibraryArgs<LocationCreateArgs>| async move {
				let (create_args, library) = arg.get_library(&ctx).await?;

				let location = create_args.create(&library).await?;
				scan_location(&library, &location).await?;

				Ok(location)
			},
		)
		.mutation(
			"update",
			|ctx, arg: LibraryArgs<LocationUpdateArgs>| async move {
				let (update_args, library) = arg.get_library(&ctx).await?;
				update_args.update(&library).await.map_err(Into::into)
			},
		)
		.mutation("delete", |ctx, arg: LibraryArgs<i32>| async move {
			let (location_id, library) = arg.get_library(&ctx).await?;

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

			invalidate_query!(
				library,
				"locations.get": LibraryArgs<()>,
				LibraryArgs::new(library.id, ())
			);

			info!("Location {} deleted", location_id);

			Ok(())
		})
		.mutation("fullRescan", |ctx, arg: LibraryArgs<i32>| async move {
			let (location_id, library) = arg.get_library(&ctx).await?;

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
		.mutation("quickRescan", |_, _: LibraryArgs<()>| todo!())
		.merge("indexer_rules", mount_indexer_rule_routes())
}

fn mount_indexer_rule_routes() -> RouterBuilder {
	<RouterBuilder>::new()
		.mutation(
			"create",
			|ctx, arg: LibraryArgs<IndexerRuleCreateArgs>| async move {
				let (create_args, library) = arg.get_library(&ctx).await?;
				create_args.create(&library).await.map_err(Into::into)
			},
		)
		.mutation("delete", |ctx, arg: LibraryArgs<i32>| async move {
			let (indexer_rule_id, library) = arg.get_library(&ctx).await?;

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
		.query("get", |ctx, arg: LibraryArgs<i32>| async move {
			let (indexer_rule_id, library) = arg.get_library(&ctx).await?;
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
		.query("list", |ctx, arg: LibraryArgs<()>| async move {
			let (_, library) = arg.get_library(&ctx).await?;

			library
				.db
				.indexer_rule()
				.find_many(vec![])
				.exec()
				.await
				.map_err(Into::into)
		})
}
