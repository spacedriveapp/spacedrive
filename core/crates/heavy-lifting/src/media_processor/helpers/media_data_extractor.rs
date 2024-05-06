use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_metadata::ImageMetadata;
use sd_prisma::prisma::media_data;

use once_cell::sync::Lazy;

pub static FILTERED_IMAGE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
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
