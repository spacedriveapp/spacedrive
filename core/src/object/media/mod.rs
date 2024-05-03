pub mod exif_data_extractor;
pub mod old_media_processor;
pub mod old_thumbnail;

pub use old_media_processor::OldMediaProcessorJobInit;
use sd_media_metadata::ExifMetadata;
use sd_prisma::prisma::exif_data::*;

use self::exif_data_extractor::ExifDataError;

pub fn exif_data_image_to_query(
	mdi: ExifMetadata,
	object_id: object_id::Type,
) -> Result<CreateUnchecked, ExifDataError> {
	Ok(CreateUnchecked {
		object_id,
		_params: vec![
			camera_data::set(serde_json::to_vec(&mdi.camera_data).ok()),
			media_date::set(serde_json::to_vec(&mdi.date_taken).ok()),
			resolution::set(serde_json::to_vec(&mdi.resolution).ok()),
			media_location::set(serde_json::to_vec(&mdi.location).ok()),
			artist::set(mdi.artist),
			description::set(mdi.description),
			copyright::set(mdi.copyright),
			exif_version::set(mdi.exif_version),
			epoch_time::set(mdi.date_taken.map(|x| x.unix_timestamp())),
		],
	})
}

pub fn exif_data_image_to_query_params(
	mdi: ExifMetadata,
) -> (Vec<(&'static str, rmpv::Value)>, Vec<SetParam>) {
	use sd_sync::option_sync_db_entry;
	use sd_utils::chain_optional_iter;

	chain_optional_iter(
		[],
		[
			option_sync_db_entry!(serde_json::to_vec(&mdi.camera_data).ok(), camera_data),
			option_sync_db_entry!(serde_json::to_vec(&mdi.date_taken).ok(), media_date),
			option_sync_db_entry!(serde_json::to_vec(&mdi.location).ok(), media_location),
			option_sync_db_entry!(mdi.artist, artist),
			option_sync_db_entry!(mdi.description, description),
			option_sync_db_entry!(mdi.copyright, copyright),
			option_sync_db_entry!(mdi.exif_version, exif_version),
		],
	)
	.into_iter()
	.unzip()
}

pub fn exif_data_image_from_prisma_data(
	data: sd_prisma::prisma::exif_data::Data,
) -> Result<ExifMetadata, ExifDataError> {
	Ok(ExifMetadata {
		camera_data: from_slice_option_to_option(data.camera_data).unwrap_or_default(),
		date_taken: from_slice_option_to_option(data.media_date).unwrap_or_default(),
		resolution: from_slice_option_to_option(data.resolution).unwrap_or_default(),
		location: from_slice_option_to_option(data.media_location),
		artist: data.artist,
		description: data.description,
		copyright: data.copyright,
		exif_version: data.exif_version,
	})
}

#[must_use]
fn from_slice_option_to_option<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: Option<Vec<u8>>,
) -> Option<T> {
	value
		.map(|x| serde_json::from_slice(&x).ok())
		.unwrap_or_default()
}
