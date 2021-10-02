use crate::db;
use crate::db::entity::file;
use crate::filesystem::checksum;
use crate::util::time;
use sea_orm::entity::*;
use sea_orm::QueryFilter;
use std::ffi::OsStr;
use std::{fs, io, path, path::PathBuf};
// use crate::error::*;
use sea_orm::DbErr;
use snafu::{ResultExt, Snafu};

#[derive(Debug, Snafu)]
pub enum Error {
  #[snafu(display("Unable to read configuration from {}: {}", path, source))]
  DatabaseConnectionResult { source: DbErr, path: String },

  #[snafu(display("Unable to read file metadata {}: {}", path.display(), source))]
  ReadMetadata { source: io::Error, path: PathBuf },

  #[snafu(display("Unable to check existing file {}: {}", meta_checksum, source))]
  ExistingFileError {
    source: DbErr,
    meta_checksum: String,
  },

  #[snafu(display("Unable to check existing file {}", source))]
  MetaChecksumError { source: io::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub async fn path(path: &str) -> Result<()> {
  let db = db::connection::get_connection()
    .await
    .context(DatabaseConnectionResult { path })?;

  let path_buff = path::PathBuf::from(path);
  let metadata = fs::metadata(&path).context(ReadMetadata { path: &path_buff })?;

  if metadata.is_dir() {
    // read_dir().await?;
  } else {
    read_file(&path, db, path_buff, metadata).await?;
  }
  Ok(())
}

// Read a file from path returning the File struct
// Generates meta checksum and extracts metadata
pub async fn read_file(
  path: &str,
  db: sea_orm::DatabaseConnection,
  path_buff: path::PathBuf,
  metadata: fs::Metadata,
) -> Result<()> {
  let size = metadata.len();
  let meta_checksum =
    checksum::create_meta_checksum(path.to_owned(), size).context(MetaChecksumError {})?;

  let existing_files = file::Entity::find()
    .filter(file::Column::MetaChecksum.contains(&meta_checksum))
    .all(&db)
    .await
    .context(ExistingFileError {
      meta_checksum: &meta_checksum,
    })?;

  if existing_files.len() == 0 {
    let file = file::ActiveModel {
      meta_checksum: Set(meta_checksum),
      name: Set(extract_name(path_buff.file_name())),
      extension: Set(extract_name(path_buff.extension())),
      uri: Set(path.to_owned()),
      size_in_bytes: Set(size.to_string()),
      date_created: Set(Some(
        time::system_time_to_date_time(metadata.created()).unwrap(),
      )),
      date_modified: Set(Some(
        time::system_time_to_date_time(metadata.modified()).unwrap(),
      )),
      date_indexed: Set(Some(
        time::system_time_to_date_time(metadata.modified()).unwrap(),
      )),
      ..Default::default()
    };

    let file = file
      .save(&db)
      .await
      .map_err(|error| println!("Failed to read file: {}", error))
      .unwrap();

    println!("FILE: {:?}", file);

    Ok(())
  } else {
    let file = &existing_files[0];
    Ok(())
  }
}

// extract name from OsStr returned by PathBuff
fn extract_name(os_string: Option<&OsStr>) -> String {
  os_string
    .unwrap_or_default()
    .to_str()
    .unwrap_or_default()
    .to_owned()
}
