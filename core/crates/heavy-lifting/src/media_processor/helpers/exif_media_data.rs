use crate::media_processor::{self, media_data_extractor};

use sd_core_prisma_helpers::ObjectPubId;
use sd_core_sync::Manager as SyncManager;

use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_metadata::ExifMetadata;
use sd_prisma::{
	prisma::{exif_data, object, PrismaClient},
	prisma_sync,
};
use sd_sync::{option_sync_db_entry, OperationFactory};
use sd_utils::chain_optional_iter;

use std::path::Path;

use futures_concurrency::future::TryJoin;
use once_cell::sync::Lazy;

use super::from_slice_option_to_option;

pub static AVAILABLE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.copied()
		.filter(|&ext| can_extract(ext))
		.map(Extension::Image)
		.collect()
});

#[must_use]
pub const fn can_extract(image_extension: ImageExtension) -> bool {
	use ImageExtension::{
		Avci, Avcs, Avif, Dng, Heic, Heif, Heifs, Hif, Jpeg, Jpg, Png, Tiff, Webp,
	};
	matches!(
		image_extension,
		Tiff | Dng | Jpeg | Jpg | Heif | Heifs | Heic | Avif | Avcs | Avci | Hif | Png | Webp
	)
}

#[must_use]
fn to_query(
	ExifMetadata {
		resolution,
		date_taken,
		location,
		camera_data,
		artist,
		description,
		copyright,
		exif_version,
	}: ExifMetadata,
	object_id: exif_data::object_id::Type,
) -> (Vec<(&'static str, rmpv::Value)>, exif_data::Create) {
	let (sync_params, db_params) = chain_optional_iter(
		[],
		[
			option_sync_db_entry!(
				serde_json::to_vec(&camera_data).ok(),
				exif_data::camera_data
			),
			option_sync_db_entry!(serde_json::to_vec(&date_taken).ok(), exif_data::media_date),
			option_sync_db_entry!(serde_json::to_vec(&resolution).ok(), exif_data::resolution),
			option_sync_db_entry!(
				serde_json::to_vec(&location).ok(),
				exif_data::media_location
			),
			option_sync_db_entry!(artist, exif_data::artist),
			option_sync_db_entry!(description, exif_data::description),
			option_sync_db_entry!(copyright, exif_data::copyright),
			option_sync_db_entry!(exif_version, exif_data::exif_version),
			option_sync_db_entry!(
				date_taken.map(|x| x.unix_timestamp()),
				exif_data::epoch_time
			),
		],
	)
	.into_iter()
	.unzip();

	(
		sync_params,
		exif_data::Create {
			object: object::id::equals(object_id),
			_params: db_params,
		},
	)
}

pub async fn extract(
	path: impl AsRef<Path> + Send,
) -> Result<Option<ExifMetadata>, media_processor::NonCriticalMediaProcessorError> {
	let path = path.as_ref();

	ExifMetadata::from_path(&path).await.map_err(|e| {
		media_data_extractor::NonCriticalMediaDataExtractorError::FailedToExtractImageMediaData(
			path.to_path_buf(),
			e.to_string(),
		)
		.into()
	})
}

pub async fn save(
	exif_datas: impl IntoIterator<Item = (ExifMetadata, object::id::Type, ObjectPubId)> + Send,
	db: &PrismaClient,
	sync: &SyncManager,
) -> Result<u64, sd_core_sync::Error> {
	exif_datas
		.into_iter()
		.map(|(exif_data, object_id, object_pub_id)| async move {
			let (sync_params, create) = to_query(exif_data, object_id);
			let db_params = create._params.clone();

			sync.write_ops(
				db,
				(
					sync.shared_create(
						prisma_sync::exif_data::SyncId {
							object: prisma_sync::object::SyncId {
								pub_id: object_pub_id.into(),
							},
						},
						sync_params,
					),
					db.exif_data()
						.upsert(exif_data::object_id::equals(object_id), create, db_params)
						.select(exif_data::select!({ id })),
				),
			)
			.await
		})
		.collect::<Vec<_>>()
		.try_join()
		.await
		.map(|created_vec| created_vec.len() as u64)
}

#[must_use]
pub fn from_prisma_data(
	exif_data::Data {
		resolution,
		media_date,
		media_location,
		camera_data,
		artist,
		description,
		copyright,
		exif_version,
		..
	}: exif_data::Data,
) -> ExifMetadata {
	ExifMetadata {
		camera_data: from_slice_option_to_option(camera_data).unwrap_or_default(),
		date_taken: from_slice_option_to_option(media_date).unwrap_or_default(),
		resolution: from_slice_option_to_option(resolution).unwrap_or_default(),
		location: from_slice_option_to_option(media_location),
		artist,
		description,
		copyright,
		exif_version,
	}
}
