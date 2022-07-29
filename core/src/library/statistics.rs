use crate::{
	prisma::{self, statistics},
	sys::{get_volumes, save_volume},
};
use chrono::Utc;
use fs_extra::dir::get_size;
use tokio::fs;

use super::LibraryContext;

pub async fn calculate_statistics(
	ctx: &LibraryContext,
) -> Result<statistics::Data, prisma::QueryError> {
	let _statistics = ctx
		.db
		.statistics()
		.find_unique(statistics::id::equals(ctx.node_local_id))
		.exec()
		.await?;

	// TODO: get from database, not sys
	let volumes = get_volumes();
	save_volume(ctx).await.unwrap(); // TODO: Error handling

	let mut available_capacity: u64 = 0;
	let mut total_capacity: u64 = 0;
	if volumes.is_ok() {
		for volume in volumes.unwrap() {
			total_capacity += volume.total_capacity;
			available_capacity += volume.available_capacity;
		}
	}

	let library_db_size = match fs::metadata(ctx.config().data_directory()).await {
		Ok(metadata) => metadata.len(),
		Err(_) => 0,
	};

	let thumbnail_folder_size = get_size(ctx.config().data_directory().join("thumbnails"));

	let params = vec![
		statistics::id::set(1), // Each library is a database so only one of these ever exists
		statistics::date_captured::set(Utc::now().into()),
		statistics::total_file_count::set(0),
		statistics::library_db_size::set(library_db_size.to_string()),
		statistics::total_bytes_used::set(0.to_string()),
		statistics::total_bytes_capacity::set(total_capacity.to_string()),
		statistics::total_unique_bytes::set(0.to_string()),
		statistics::total_bytes_free::set(available_capacity.to_string()),
		statistics::preview_media_bytes::set(thumbnail_folder_size.unwrap_or(0).to_string()),
	];

	ctx.db
		.statistics()
		.upsert(
			statistics::id::equals(1), // Each library is a database so only one of these ever exists
			params.clone(),
			params,
		)
		.exec()
		.await
}
