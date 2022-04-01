use crate::{file::DirectoryWithContents, prisma::FilePath, CoreContext};

use super::FileError;

pub async fn open_dir(ctx: &CoreContext, path: &str) -> Result<DirectoryWithContents, FileError> {
  let db = &ctx.database;

  println!("getting files... {:?}", &path);

  let directory = db
    .file_path()
    .find_first(vec![
      FilePath::materialized_path().equals(path.into()),
      FilePath::is_dir().equals(true),
    ])
    .exec()
    .await?
    .ok_or(FileError::FileNotFound(path.to_string()))?;

  let files = db
    .file_path()
    .find_many(vec![FilePath::parent_id().equals(directory.id)])
    .exec()
    .await?;

  Ok(DirectoryWithContents {
    directory: directory.into(),
    contents: files.into_iter().map(|l| l.into()).collect(),
  })
}
