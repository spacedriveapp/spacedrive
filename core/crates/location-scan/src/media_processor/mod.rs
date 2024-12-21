use crate::OuterContext;

use sd_core_file_helper::IsolatedFilePathData;
use sd_core_job_system::UpdateEvent;
use sd_core_prisma_helpers::file_path_for_media_processor;

use sd_file_ext::extensions::Extension;
use sd_prisma::prisma::{file_path, object, PrismaClient};

use std::{collections::HashMap, fmt};

use prisma_client_rust::{raw, PrismaValue};
use serde::Deserialize;

use sd_core_job_errors::media_processor::Error;

mod helpers;
pub mod job;
mod shallow;
mod tasks;

pub use tasks::{
	media_data_extractor::{self, MediaDataExtractor},
	thumbnailer::{self, Thumbnailer},
};

pub use helpers::{
	exif_media_data, ffmpeg_media_data,
	thumbnailer::{
		can_generate_thumbnail_for_document, can_generate_thumbnail_for_image,
		generate_single_thumbnail, get_shard_hex, get_thumbnails_directory, GenerateThumbnailArgs,
		ThumbKey, ThumbnailKind, WEBP_EXTENSION,
	},
};

#[cfg(feature = "ffmpeg")]
pub use helpers::thumbnailer::can_generate_thumbnail_for_video;

pub use shallow::shallow;
use thumbnailer::NewThumbnailReporter;

const BATCH_SIZE: usize = 10;

#[derive(Clone)]
pub struct NewThumbnailsReporter<OuterCtx: OuterContext> {
	pub ctx: OuterCtx,
}

impl<OuterCtx: OuterContext> fmt::Debug for NewThumbnailsReporter<OuterCtx> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("NewThumbnailsReporter").finish()
	}
}

impl<OuterCtx: OuterContext> NewThumbnailReporter for NewThumbnailsReporter<OuterCtx> {
	fn new_thumbnail(&self, thumb_key: ThumbKey) {
		self.ctx
			.report_update(UpdateEvent::NewThumbnail { thumb_key });
	}
}

#[derive(Deserialize)]
struct RawFilePathForMediaProcessor {
	id: file_path::id::Type,
	materialized_path: file_path::materialized_path::Type,
	is_dir: file_path::is_dir::Type,
	name: file_path::name::Type,
	extension: file_path::extension::Type,
	cas_id: file_path::cas_id::Type,
	object_id: object::id::Type,
	object_pub_id: object::pub_id::Type,
}

impl From<RawFilePathForMediaProcessor> for file_path_for_media_processor::Data {
	fn from(
		RawFilePathForMediaProcessor {
			id,
			materialized_path,
			is_dir,
			name,
			extension,
			cas_id,
			object_id,
			object_pub_id,
		}: RawFilePathForMediaProcessor,
	) -> Self {
		Self {
			id,
			materialized_path,
			is_dir,
			name,
			extension,
			cas_id,
			object: Some(file_path_for_media_processor::object::Data {
				id: object_id,
				pub_id: object_pub_id,
			}),
		}
	}
}

async fn get_direct_children_files_by_extensions(
	parent_iso_file_path: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
	db: &PrismaClient,
) -> Result<Vec<file_path_for_media_processor::Data>, Error> {
	// FIXME: Had to use format! macro because PCR doesn't support IN with Vec for SQLite
	// We have no data coming from the user, so this is sql injection safe
	let unique_by_object_id = db
		._query_raw::<RawFilePathForMediaProcessor>(raw!(
			&format!(
				"SELECT
				file_path.id,
				file_path.materialized_path,
				file_path.is_dir,
				file_path.name,
				file_path.extension,
				file_path.cas_id,
				object.id as 'object_id',
				object.pub_id as 'object_pub_id'
			FROM file_path
			INNER JOIN object ON object.id = file_path.object_id
			WHERE
				location_id={{}}
				AND cas_id IS NOT NULL
				AND LOWER(extension) IN ({})
				AND materialized_path = {{}}
			ORDER BY name ASC",
				extensions
					.iter()
					.map(|ext| format!("LOWER('{ext}')"))
					.collect::<Vec<_>>()
					.join(",")
			),
			PrismaValue::Int(parent_iso_file_path.location_id()),
			PrismaValue::String(
				parent_iso_file_path
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory")
			)
		))
		.exec()
		.await?
		.into_iter()
		.map(|raw_file_path| (raw_file_path.object_id, raw_file_path))
		.collect::<HashMap<_, _>>();

	Ok(unique_by_object_id.into_values().map(Into::into).collect())
}
