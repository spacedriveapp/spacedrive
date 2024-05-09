use crate::media_processor::{self, media_data_extractor};

use sd_file_ext::extensions::{Extension, ImageExtension, ALL_IMAGE_EXTENSIONS};
use sd_media_metadata::ExifMetadata;
use sd_prisma::prisma::{exif_data, object, PrismaClient};

use std::path::Path;

use once_cell::sync::Lazy;

pub static AVAILABLE_EXTENSIONS: Lazy<Vec<Extension>> = Lazy::new(|| {
	ALL_IMAGE_EXTENSIONS
		.iter()
		.copied()
		.filter(|&ext| can_extract(ext))
		.map(Extension::Image)
		.collect()
});

pub const fn can_extract(image_extension: ImageExtension) -> bool {
	use ImageExtension::{
		Avci, Avcs, Avif, Dng, Heic, Heif, Heifs, Hif, Jpeg, Jpg, Png, Tiff, Webp,
	};
	matches!(
		image_extension,
		Tiff | Dng | Jpeg | Jpg | Heif | Heifs | Heic | Avif | Avcs | Avci | Hif | Png | Webp
	)
}

pub fn to_query(
	mdi: ExifMetadata,
	object_id: exif_data::object_id::Type,
) -> exif_data::CreateUnchecked {
	exif_data::CreateUnchecked {
		object_id,
		_params: vec![
			exif_data::camera_data::set(serde_json::to_vec(&mdi.camera_data).ok()),
			exif_data::media_date::set(serde_json::to_vec(&mdi.date_taken).ok()),
			exif_data::resolution::set(serde_json::to_vec(&mdi.resolution).ok()),
			exif_data::media_location::set(serde_json::to_vec(&mdi.location).ok()),
			exif_data::artist::set(mdi.artist),
			exif_data::description::set(mdi.description),
			exif_data::copyright::set(mdi.copyright),
			exif_data::exif_version::set(mdi.exif_version),
			exif_data::epoch_time::set(mdi.date_taken.map(|x| x.unix_timestamp())),
		],
	}
}

pub async fn extract(
	path: impl AsRef<Path> + Send,
) -> Result<Option<ExifMetadata>, media_processor::NonCriticalError> {
	let path = path.as_ref();

	ExifMetadata::from_path(&path).await.map_err(|e| {
		media_data_extractor::NonCriticalError::FailedToExtractImageMediaData(
			path.to_path_buf(),
			e.to_string(),
		)
		.into()
	})
}

pub async fn save(
	media_datas: Vec<(ExifMetadata, object::id::Type)>,
	db: &PrismaClient,
) -> Result<u64, media_processor::Error> {
	db.exif_data()
		.create_many(
			media_datas
				.into_iter()
				.map(|(exif_data, object_id)| to_query(exif_data, object_id))
				.collect(),
		)
		.skip_duplicates()
		.exec()
		.await
		.map(|created| {
			#[allow(clippy::cast_sign_loss)]
			{
				created as u64
			}
		})
		.map_err(Into::into)
}
