pub mod media_data_extractor;
pub mod media_processor;
pub mod thumbnail;

pub use media_processor::MediaProcessorJobInit;
use sd_media_data::MediaDataImage;
use sd_prisma::prisma::media_data;

use self::media_data_extractor::MediaDataError;

pub fn media_data_image_to_query(
	mdi: MediaDataImage,
	object_id: sd_prisma::prisma::media_data::object_id::Type,
) -> Result<sd_prisma::prisma::media_data::CreateUnchecked, MediaDataError> {
	Ok(media_data::CreateUnchecked {
		object_id,
		dimensions: serde_json::to_vec(&mdi.dimensions).map_err(sd_media_data::Error::Serde)?,
		media_date: serde_json::to_vec(&mdi.date_taken).map_err(sd_media_data::Error::Serde)?,
		camera_data: serde_json::to_vec(&mdi.camera_data).map_err(sd_media_data::Error::Serde)?,
		_params: vec![
			media_data::media_location::set(serde_json::to_vec(&mdi.location).ok()),
			media_data::artist::set(serde_json::to_vec(&mdi.artist).ok()),
			media_data::description::set(serde_json::to_vec(&mdi.description).ok()),
			media_data::copyright::set(serde_json::to_vec(&mdi.copyright).ok()),
			media_data::exif_version::set(serde_json::to_vec(&mdi.exif_version).ok()),
		],
	})
}

pub fn media_data_image_from_prisma_data(
	data: sd_prisma::prisma::media_data::Data,
) -> Result<MediaDataImage, MediaDataError> {
	Ok(MediaDataImage {
		dimensions: serde_json::from_slice(&data.dimensions)
			.map_err(sd_media_data::Error::Serde)?,
		camera_data: serde_json::from_slice(&data.camera_data)
			.map_err(sd_media_data::Error::Serde)?,
		date_taken: serde_json::from_slice(&data.media_date)
			.map_err(sd_media_data::Error::Serde)?,
		description: from_slice_option_to_option(data.description),
		copyright: from_slice_option_to_option(data.copyright),
		artist: from_slice_option_to_option(data.artist),
		location: from_slice_option_to_option(data.media_location),
		exif_version: from_slice_option_to_option(data.exif_version),
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
