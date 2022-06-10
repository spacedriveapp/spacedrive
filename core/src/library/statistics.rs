use crate::{
	prisma::{library, library_statistics::*},
	sys::Volume,
};
use fs_extra::dir::get_size;
use serde::{Deserialize, Serialize};
use std::fs;
use ts_rs::TS;

use super::{LibraryContext, LibraryError};

#[derive(Debug, Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub struct Statistics {
	pub total_file_count: i32,
	pub total_bytes_used: String,
	pub total_bytes_capacity: String,
	pub total_bytes_free: String,
	pub total_unique_bytes: String,
	pub preview_media_bytes: String,
	pub library_db_size: String,
}

impl Into<Statistics> for Data {
	fn into(self) -> Statistics {
		Statistics {
			total_file_count: self.total_file_count,
			total_bytes_used: self.total_bytes_used,
			total_bytes_capacity: self.total_bytes_capacity,
			total_bytes_free: self.total_bytes_free,
			total_unique_bytes: self.total_unique_bytes,
			preview_media_bytes: self.preview_media_bytes,
			library_db_size: String::new(),
		}
	}
}

impl Default for Statistics {
	fn default() -> Self {
		Self {
			total_file_count: 0,
			total_bytes_used: String::new(),
			total_bytes_capacity: String::new(),
			total_bytes_free: String::new(),
			total_unique_bytes: String::new(),
			preview_media_bytes: String::new(),
			library_db_size: String::new(),
		}
	}
}

impl Statistics {
	pub async fn retrieve(ctx: &LibraryContext) -> Result<Statistics, LibraryError> {
		let library_statistics_db = match ctx
			.db
			.library_statistics()
			.find_unique(id::equals(ctx.node_local_id))
			.exec()
			.await?
		{
			Some(library_statistics_db) => library_statistics_db.into(),
			// create the default values if database has no entry
			None => Statistics::default(),
		};
		Ok(library_statistics_db.into())
	}

	pub async fn calculate(ctx: &LibraryContext) -> Result<Statistics, LibraryError> {
		// get library from db
		let library = ctx
			.db
			.library()
			.find_unique(library::pub_id::equals(ctx.id.to_string()))
			.exec()
			.await?;

		if library.is_none() {
			return Err(LibraryError::LibraryNotFound);
		}

		let _library_statistics = ctx
			.db
			.library_statistics()
			.find_unique(id::equals(ctx.node_local_id))
			.exec()
			.await?;

		// TODO: get from database, not sys
		let volumes = Volume::get_volumes();
		Volume::save(&ctx).await?;

		// println!("{:?}", volumes);

		let mut available_capacity: u64 = 0;
		let mut total_capacity: u64 = 0;
		if volumes.is_ok() {
			for volume in volumes.unwrap() {
				total_capacity += volume.total_capacity;
				available_capacity += volume.available_capacity;
			}
		}

		let library_db_size = match fs::metadata(ctx.config().data_directory()) {
			Ok(metadata) => metadata.len(),
			Err(_) => 0,
		};

		let mut thumbsnails_dir = ctx.config().data_directory();
		thumbsnails_dir.push("thumbnails");

		let thumbnail_folder_size = get_size(&thumbsnails_dir);

		let statistics = Statistics {
			library_db_size: library_db_size.to_string(),
			total_bytes_free: available_capacity.to_string(),
			total_bytes_capacity: total_capacity.to_string(),
			preview_media_bytes: thumbnail_folder_size.unwrap_or(0).to_string(),
			..Statistics::default()
		};

		let library_local_id = match library {
			Some(library) => library.id,
			None => ctx.node_local_id,
		};

		ctx.db
			.library_statistics()
			.upsert(
				library_id::equals(library_local_id),
				(
					library_id::set(library_local_id),
					vec![library_db_size::set(statistics.library_db_size.clone())],
				),
				vec![
					total_file_count::set(statistics.total_file_count.clone()),
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
