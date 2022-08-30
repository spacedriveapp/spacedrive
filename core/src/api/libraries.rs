use chrono::Utc;
use fs_extra::dir::get_size;
use rspc::Type;
use serde::Deserialize;
use tokio::fs;
use uuid::Uuid;

use crate::{
	library::LibraryConfig,
	prisma::statistics,
	sys::{get_volumes, save_volume},
};

use super::{LibraryArgs, RouterBuilder};

#[derive(Type, Deserialize)]
pub struct EditLibraryArgs {
	pub id: Uuid,
	pub name: Option<String>,
	pub description: Option<String>,
}

pub(crate) fn mount() -> RouterBuilder {
	<RouterBuilder>::new()
		.query("get", |ctx, _: ()| async move {
			ctx.library_manager.get_all_libraries_config().await
		})
		.query("getStatistics", |ctx, arg: LibraryArgs<()>| async move {
			let (_, library) = arg.get_library(&ctx).await?;

			let _statistics = library
				.db
				.statistics()
				.find_unique(statistics::id::equals(library.node_local_id))
				.exec()
				.await?;

			// TODO: get from database, not sys
			let volumes = get_volumes();
			save_volume(&library).await?;

			let mut available_capacity: u64 = 0;
			let mut total_capacity: u64 = 0;
			if volumes.is_ok() {
				for volume in volumes? {
					total_capacity += volume.total_capacity;
					available_capacity += volume.available_capacity;
				}
			}

			let library_db_size = match fs::metadata(library.config().data_directory()).await {
				Ok(metadata) => metadata.len(),
				Err(_) => 0,
			};

			let thumbnail_folder_size =
				get_size(library.config().data_directory().join("thumbnails"));

			use statistics::*;
			let params = vec![
				id::set(1), // Each library is a database so only one of these ever exists
				date_captured::set(Utc::now().into()),
				total_file_count::set(0),
				library_db_size::set(library_db_size.to_string()),
				total_bytes_used::set(0.to_string()),
				total_bytes_capacity::set(total_capacity.to_string()),
				total_unique_bytes::set(0.to_string()),
				total_bytes_free::set(available_capacity.to_string()),
				preview_media_bytes::set(thumbnail_folder_size.unwrap_or(0).to_string()),
			];

			Ok(library
				.db
				.statistics()
				.upsert(
					statistics::id::equals(1), // Each library is a database so only one of these ever exists
					params.clone(),
					params,
				)
				.exec()
				.await?)
		})
		.mutation("create", |ctx, name: String| async move {
			Ok(ctx
				.library_manager
				.create(LibraryConfig {
					name: name.to_string(),
					..Default::default()
				})
				.await?)
		})
		.mutation("edit", |ctx, args: EditLibraryArgs| async move {
			Ok(ctx
				.library_manager
				.edit(args.id, args.name, args.description)
				.await?)
		})
		.mutation("delete", |ctx, id: Uuid| async move {
			Ok(ctx.library_manager.delete_library(id).await?)
		})
}
