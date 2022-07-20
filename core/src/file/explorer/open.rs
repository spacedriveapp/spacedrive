use crate::{
	encode::THUMBNAIL_CACHE_DIR_NAME,
	file::{DirectoryWithContents, FileError, FilePath},
	library::LibraryContext,
	prisma::file_path,
	sys::get_location,
};
use std::path::Path;
use tracing::info;

pub async fn open_dir(
	ctx: &LibraryContext,
	location_id: i32,
	path: impl AsRef<Path>,
) -> Result<DirectoryWithContents, FileError> {
	// get location
	let location = get_location(ctx, location_id).await?;

	let path_str = path.as_ref().to_string_lossy().to_string();

	let directory = ctx
		.db
		.file_path()
		.find_first(vec![
			file_path::location_id::equals(Some(location.id)),
			file_path::materialized_path::equals(path_str),
			file_path::is_dir::equals(true),
		])
		.exec()
		.await?
		.ok_or_else(|| FileError::DirectoryNotFound(path.as_ref().to_path_buf()))?;

	info!("DIRECTORY: {:?}", directory);

	let mut file_paths: Vec<FilePath> = ctx
		.db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(Some(location.id)),
			file_path::parent_id::equals(Some(directory.id)),
		])
		.with(file_path::file::fetch())
		.exec()
		.await?
		.into_iter()
		.map(Into::into)
		.collect();

	for file_path in &mut file_paths {
		if let Some(file) = &mut file_path.file {
			let thumb_path = ctx
				.config()
				.data_directory()
				.join(THUMBNAIL_CACHE_DIR_NAME)
				.join(location.id.to_string())
				.join(&file.cas_id)
				.with_extension("webp");

			file.has_thumbnail = thumb_path.exists();
		}
	}

	Ok(DirectoryWithContents {
		directory: directory.into(),
		contents: file_paths,
	})
}

// pub async fn open_tag(ctx: &LibraryContext, tag_id: i32) -> Result<TagWithFiles, TagError> {
// 	let tag: Tag = ctx
// 		.db
// 		.tag()
// 		.find_unique(tag::id::equals(tag_id))
// 		.exec()
// 		.await?
// 		.unwrap()
// 		.into();

// 	let files_with_tag: Vec<TagOnFile> = ctx
// 		.db
// 		.tag_on_file()
// 		.find_many(vec![tag_on_file::tag_id::equals(tag_id)])
// 		.exec()
// 		.await?
// 		.into_iter()
// 		.map(Into::into)
// 		.collect();

// 	Ok(TagWithFiles {
// 		tag,
// 		files_with_tag,
// 	})
// }
