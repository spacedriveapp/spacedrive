use crate::{
	encode::THUMBNAIL_CACHE_DIR_NAME,
	file::{DirectoryWithContents, FileError, FilePath},
	prisma::file_path,
	sys::get_location,
	CoreContext,
};
use std::path::Path;

pub async fn open_dir(
	ctx: &CoreContext,
	location_id: &i32,
	path: &str,
) -> Result<DirectoryWithContents, FileError> {
	let db = &ctx.database;
	let config = ctx.config.get().await;

	// get location
	let location = get_location(ctx, location_id.clone()).await?;

	let directory = db
		.file_path()
		.find_first(vec![
			file_path::location_id::equals(location.id),
			file_path::materialized_path::equals(path.into()),
			file_path::is_dir::equals(true),
		])
		.exec()
		.await?
		.ok_or(FileError::DirectoryNotFound(path.to_string()))?;

	println!("DIRECTORY: {:?}", directory);

	let mut file_paths: Vec<FilePath> = db
		.file_path()
		.find_many(vec![
			file_path::location_id::equals(location.id),
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
			let thumb_path = Path::new(&config.data_path)
				.join(THUMBNAIL_CACHE_DIR_NAME)
				.join(format!("{}", location.id))
				.join(file.cas_id.clone())
				.with_extension("webp");

			file.has_thumbnail = thumb_path.exists();
		}
	}

	Ok(DirectoryWithContents {
		directory: directory.into(),
		contents: file_paths,
	})
}
