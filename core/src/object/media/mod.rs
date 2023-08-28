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
			dimensions::set(serde_json::to_vec(&mdi.dimensions).ok()),
			media_location::set(serde_json::to_vec(&mdi.location).ok()),
			artist::set(serde_json::to_string(&mdi.artist).ok()),
			description::set(serde_json::to_string(&mdi.description).ok()),
			copyright::set(serde_json::to_string(&mdi.copyright).ok()),
			exif_version::set(serde_json::to_string(&mdi.exif_version).ok()),
		],
	})
}

pub fn media_data_image_from_prisma_data(
	data: sd_prisma::prisma::media_data::Data,
) -> Result<ImageMetadata, MediaDataError> {
	Ok(ImageMetadata {
		dimensions: from_slice_option_to_option(data.dimensions).unwrap_or_default(),
		camera_data: from_slice_option_to_option(data.camera_data).unwrap_or_default(),
		date_taken: from_slice_option_to_option(data.media_date).unwrap_or_default(),
		description: from_string_option_to_option(data.description),
		copyright: from_string_option_to_option(data.copyright),
		artist: from_string_option_to_option(data.artist),
		location: from_slice_option_to_option(data.media_location),
		exif_version: from_string_option_to_option(data.exif_version),
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

#[must_use]
fn from_string_option_to_option<T: serde::Serialize + serde::de::DeserializeOwned>(
	value: Option<String>,
) -> Option<T> {
	value
		.map(|x| serde_json::from_str(&x).ok())
		.unwrap_or_default()
}
