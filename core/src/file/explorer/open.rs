use crate::{
  encode::thumb::THUMBNAIL_CACHE_DIR_NAME,
  file::{DirectoryWithContents, File, FileError},
  node::state,
  prisma::file_path,
  sys::locations::get_location,
  CoreContext,
};
use std::path::Path;

pub async fn open_dir(
  ctx: &CoreContext,
  location_id: &i32,
  path: &str,
) -> Result<DirectoryWithContents, FileError> {
  let db = &ctx.database;
  let config = state::get();

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

  let files = db
    .file_path()
    .find_many(vec![file_path::parent_id::equals(Some(directory.id))])
    .with(file_path::file::fetch())
    .exec()
    .await?;

  // convert database structs into a File
  let files: Vec<File> = files
    .into_iter()
    .map(|l| {
      let mut file: File = l.file().unwrap_or_default().unwrap().clone().into();
      file.paths.push(l.into());
      file
    })
    .collect();

  let mut contents: Vec<File> = vec![];

  for mut file in files {
    let thumb_path = Path::new(&config.data_path)
      .join(THUMBNAIL_CACHE_DIR_NAME)
      .join(format!("{}", location.id))
      .join(file.cas_id.clone())
      .with_extension("webp");

    file.has_thumbnail = thumb_path.exists();
    contents.push(file);
  }

  Ok(DirectoryWithContents {
    directory: directory.into(),
    contents,
  })
}
