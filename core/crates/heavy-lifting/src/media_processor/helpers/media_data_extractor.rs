use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_metadata::ImageMetadata;
use sd_prisma::prisma::media_data;
use sd_sync::option_sync_db_entry;
use sd_utils::chain_optional_iter;

use once_cell::sync::Lazy;
use serde::{de::DeserializeOwned, Serialize};

pub(super) static FILTERED_IMAGE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.copied()
		.filter(|&ext| can_extract_media_data_for_image(ext))
		.map(Extension::Image)
		.collect()
});

pub const fn can_extract_media_data_for_image(image_extension: ImageExtension) -> bool {
	use ImageExtension::{
		Avci, Avcs, Avif, Dng, Heic, Heif, Heifs, Hif, Jpeg, Jpg, Png, Tiff, Webp,
	};
	matches!(
		image_extension,
		Tiff | Dng | Jpeg | Jpg | Heif | Heifs | Heic | Avif | Avcs | Avci | Hif | Png | Webp
	)
}

pub fn media_data_image_to_query(
	mdi: ImageMetadata,
	object_id: media_data::object_id::Type,
) -> media_data::CreateUnchecked {
	media_data::CreateUnchecked {
		object_id,
		_params: vec![
			media_data::camera_data::set(serde_json::to_vec(&mdi.camera_data).ok()),
			media_data::media_date::set(serde_json::to_vec(&mdi.date_taken).ok()),
			media_data::resolution::set(serde_json::to_vec(&mdi.resolution).ok()),
			media_data::media_location::set(serde_json::to_vec(&mdi.location).ok()),
			media_data::artist::set(mdi.artist),
			media_data::description::set(mdi.description),
			media_data::copyright::set(mdi.copyright),
			media_data::exif_version::set(mdi.exif_version),
			media_data::epoch_time::set(mdi.date_taken.map(|x| x.unix_timestamp())),
		],
	}
}

pub fn media_data_image_to_query_params(
	mdi: ImageMetadata,
) -> (Vec<(&'static str, rmpv::Value)>, Vec<media_data::SetParam>) {
	chain_optional_iter(
		[],
		[
			option_sync_db_entry!(
				serde_json::to_vec(&mdi.camera_data).ok(),
				media_data::camera_data
			),
			option_sync_db_entry!(
				serde_json::to_vec(&mdi.date_taken).ok(),
				media_data::media_date
			),
			option_sync_db_entry!(
				serde_json::to_vec(&mdi.location).ok(),
				media_data::media_location
			),
			option_sync_db_entry!(mdi.artist, media_data::artist),
			option_sync_db_entry!(mdi.description, media_data::description),
			option_sync_db_entry!(mdi.copyright, media_data::copyright),
			option_sync_db_entry!(mdi.exif_version, media_data::exif_version),
		],
	)
	.into_iter()
	.unzip()
}

pub fn media_data_image_from_prisma_data(data: media_data::Data) -> ImageMetadata {
	ImageMetadata {
		camera_data: from_slice_option_to_option(data.camera_data).unwrap_or_default(),
		date_taken: from_slice_option_to_option(data.media_date).unwrap_or_default(),
		resolution: from_slice_option_to_option(data.resolution).unwrap_or_default(),
		location: from_slice_option_to_option(data.media_location),
		artist: data.artist,
		description: data.description,
		copyright: data.copyright,
		exif_version: data.exif_version,
	}
}

#[inline]
fn from_slice_option_to_option<T: Serialize + DeserializeOwned>(
	value: Option<Vec<u8>>,
) -> Option<T> {
	value.map_or_else(Default::default, |x| serde_json::from_slice(&x).ok())
}
