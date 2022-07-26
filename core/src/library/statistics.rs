use crate::{prisma::statistics::*, sys::Volume};
use fs_extra::dir::get_size;
use rspc::Type;
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::{LibraryContext, LibraryError};

#[derive(Debug, Serialize, Deserialize, Type, Clone, Default)]
pub struct Statistics {
	pub total_file_count: i32,
	pub total_bytes_used: String,
	pub total_bytes_capacity: String,
	pub total_bytes_free: String,
	pub total_unique_bytes: String,
	pub preview_media_bytes: String,
	pub library_db_size: String,
}

impl From<Data> for Statistics {
	fn from(data: Data) -> Self {
		Self {
			total_file_count: data.total_file_count,
			total_bytes_used: data.total_bytes_used,
			total_bytes_capacity: data.total_bytes_capacity,
			total_bytes_free: data.total_bytes_free,
			total_unique_bytes: data.total_unique_bytes,
			preview_media_bytes: data.preview_media_bytes,
			library_db_size: String::new(),
		}
	}
}

impl Statistics {
	pub async fn retrieve(ctx: &LibraryContext) -> Result<Statistics, LibraryError> {
		let library_statistics_db = ctx
			.db
			.statistics()
			.find_unique(id::equals(ctx.node_local_id))
			.exec()
			.await?
			.map_or_else(Default::default, Into::into);
		Ok(library_statistics_db)
	}

	pub async fn calculate(ctx: &LibraryContext) -> Result<Statistics, LibraryError> {
		let _statistics = ctx
			.db
			.statistics()
			.find_unique(id::equals(ctx.node_local_id))
			.exec()
			.await?;

		// TODO: get from database, not sys
		let volumes = Volume::get_volumes();
		Volume::save(ctx).await?;

		// info!("{:?}", volumes);

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

		let statistics = Statistics {
			library_db_size: library_db_size.to_string(),
			total_bytes_free: available_capacity.to_string(),
			total_bytes_capacity: total_capacity.to_string(),
			preview_media_bytes: thumbnail_folder_size.unwrap_or(0).to_string(),
			..Statistics::default()
		};

		ctx.db
			.statistics()
			.upsert(
				id::equals(1),
				vec![library_db_size::set(statistics.library_db_size.clone())],
				vec![
					total_file_count::set(statistics.total_file_count),
					total_bytes_used::set(statistics.total_bytes_used.clone()),
					total_bytes_capacity::set(statistics.total_bytes_capacity.clone()),
					total_bytes_free::set(statistics.total_bytes_free.clone()),
					total_unique_bytes::set(statistics.total_unique_bytes.clone()),
					preview_media_bytes::set(statistics.preview_media_bytes.clone()),
					library_db_size::set(statistics.library_db_size.clone()),
				],
			)
			.exec()
			.await?;

		Ok(statistics)
	}
}
