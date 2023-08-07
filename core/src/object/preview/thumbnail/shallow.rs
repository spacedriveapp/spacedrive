use super::{
	ThumbnailerError, ThumbnailerJobStep, ThumbnailerJobStepKind, FILTERED_IMAGE_EXTENSIONS,
};
use crate::{
	invalidate_query,
	job::JobError,
	library::Library,
	location::file_path_helper::{
		ensure_file_path_exists, ensure_sub_path_is_directory, ensure_sub_path_is_in_location,
		file_path_for_thumbnailer, IsolatedFilePathData,
	},
	object::preview::thumbnail,
	prisma::{file_path, location, PrismaClient},
	util::error::FileIOError,
	Node,
};
use sd_file_ext::extensions::Extension;
use std::path::{Path, PathBuf};
use thumbnail::init_thumbnail_dir;
use tokio::fs;
use tracing::{debug, trace};

#[cfg(feature = "ffmpeg")]
use super::FILTERED_VIDEO_EXTENSIONS;

pub async fn shallow_thumbnailer(
	location: &location::Data,
	sub_path: &PathBuf,
	library: &Library,
	node: &Node,
) -> Result<(), JobError> {
	let Library { db, .. } = library;

	let thumbnail_dir = init_thumbnail_dir(node.config.data_directory()).await?;

	let location_id = location.id;
	let location_path = match &location.path {
		Some(v) => PathBuf::from(v),
		None => return Ok(()),
	};

	let (path, iso_file_path) = if sub_path != Path::new("") {
		let full_path = ensure_sub_path_is_in_location(&location_path, &sub_path)
			.await
			.map_err(ThumbnailerError::from)?;
		ensure_sub_path_is_directory(&location_path, &sub_path)
			.await
			.map_err(ThumbnailerError::from)?;

		let sub_iso_file_path =
			IsolatedFilePathData::new(location_id, &location_path, &full_path, true)
				.map_err(ThumbnailerError::from)?;

		ensure_file_path_exists(
			&sub_path,
			&sub_iso_file_path,
			db,
			ThumbnailerError::SubPathNotFound,
		)
		.await?;

		(full_path, sub_iso_file_path)
	} else {
		(
			location_path.to_path_buf(),
			IsolatedFilePathData::new(location_id, &location_path, &location_path, true)
				.map_err(ThumbnailerError::from)?,
		)
	};

	debug!(
		"Searching for images in location {location_id} at path {}",
		path.display()
	);

	// create all necessary directories if they don't exist
	fs::create_dir_all(&thumbnail_dir)
		.await
		.map_err(|e| FileIOError::from((&thumbnail_dir, e)))?;

	// query database for all image files in this location that need thumbnails
	let image_files = get_files_by_extensions(
		&library.db,
		location_id,
		&iso_file_path,
		&FILTERED_IMAGE_EXTENSIONS,
		ThumbnailerJobStepKind::Image,
	)
	.await?;

	trace!("Found {:?} image files", image_files.len());

	#[cfg(feature = "ffmpeg")]
	let video_files = {
		// query database for all video files in this location that need thumbnails
		let video_files = get_files_by_extensions(
			&library.db,
			location_id,
			&iso_file_path,
			&FILTERED_VIDEO_EXTENSIONS,
			ThumbnailerJobStepKind::Video,
		)
		.await?;

		trace!("Found {:?} video files", video_files.len());

		video_files
	};

	let all_files = [
		image_files,
		#[cfg(feature = "ffmpeg")]
		video_files,
	]
	.into_iter()
	.flatten();

	for file in all_files {
		thumbnail::inner_process_step(&file, &location_path, &thumbnail_dir, location, library)
			.await?;
	}

	invalidate_query!(library, "search.paths");

	Ok(())
}

async fn get_files_by_extensions(
	db: &PrismaClient,
	location_id: location::id::Type,
	parent_isolated_file_path_data: &IsolatedFilePathData<'_>,
	extensions: &[Extension],
	kind: ThumbnailerJobStepKind,
) -> Result<Vec<ThumbnailerJobStep>, JobError> {
	Ok(db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(location_id)),
			file_path::extension::in_vec(extensions.iter().map(ToString::to_string).collect()),
			file_path::materialized_path::equals(Some(
				parent_isolated_file_path_data
					.materialized_path_for_children()
					.expect("sub path iso_file_path must be a directory"),
			)),
		])
		.select(file_path_for_thumbnailer::select())
		.exec()
		.await?
		.into_iter()
		.map(|file_path| ThumbnailerJobStep { file_path, kind })
		.collect())
}
