pub mod media_data_extractor;
pub mod media_processor;
pub mod thumbnail;

pub use media_processor::MediaProcessorJobInit;
use sd_media_metadata::ImageMetadata;
use sd_prisma::prisma::media_data::*;

use self::media_data_extractor::MediaDataError;

pub fn media_data_image_to_query(
	mdi: ImageMetadata,
	object_id: object_id::Type,
) -> Result<CreateUnchecked, MediaDataError> {
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

pub fn media_data_image_from_prisma_data(
	data: sd_prisma::prisma::media_data::Data,
) -> Result<ImageMetadata, MediaDataError> {
	Ok(ImageMetadata {
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
