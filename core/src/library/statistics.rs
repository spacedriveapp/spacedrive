use crate::{
  prisma::{library, library_statistics::*},
  state::client,
  CoreContext,
};
use serde::{Deserialize, Serialize};
use std::fs;
use ts_rs::TS;

use super::LibraryError;

#[derive(Debug, Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub struct Statistics {
  pub total_file_count: i32,
  pub total_bytes_used: String,
  pub total_bytes_capacity: String,
  pub total_bytes_free: String,
  pub total_unique_bytes: String,
  pub preview_media_bytes: String,
  pub library_db_size: String,
}

impl Into<Statistics> for Data {
  fn into(self) -> Statistics {
    Statistics {
      total_file_count: self.total_file_count,
      total_bytes_used: self.total_bytes_used,
      total_bytes_capacity: self.total_bytes_capacity,
      total_bytes_free: self.total_bytes_free,
      total_unique_bytes: self.total_unique_bytes,
      preview_media_bytes: self.preview_media_bytes,
      library_db_size: String::new(),
    }
  }
}

impl Default for Statistics {
  fn default() -> Self {
    Self {
      total_file_count: 0,
      total_bytes_used: String::new(),
      total_bytes_capacity: String::new(),
      total_bytes_free: String::new(),
      total_unique_bytes: String::new(),
      preview_media_bytes: String::new(),
      library_db_size: String::new(),
    }
  }
}

impl Statistics {
  pub async fn retrieve(ctx: &CoreContext) -> Result<Statistics, LibraryError> {
    let config = client::get();
    let db = &ctx.database;
    let library_data = config.get_current_library();

    let library_statistics_db = match db
      .library_statistics()
      .find_unique(id::equals(library_data.library_id))
      .exec()
      .await?
    {
      Some(library_statistics_db) => library_statistics_db.into(),
      // create the default values if database has no entry
      None => Statistics::default(),
    };
    Ok(library_statistics_db.into())
  }
  pub async fn calculate(ctx: &CoreContext) -> Result<Statistics, LibraryError> {
    let config = client::get();
    let db = &ctx.database;
    // get library from client state
    let library_data = config.get_current_library();

    println!(
      "Calculating library statistics {:?}",
      library_data.library_uuid
    );
    // get library from db
    let library = db
      .library()
      .find_unique(library::pub_id::equals(
        library_data.library_uuid.to_string(),
      ))
      .exec()
      .await?;

    if library.is_none() {
      return Err(LibraryError::LibraryNotFound);
    }

    let library_statistics = db
      .library_statistics()
      .find_unique(id::equals(library_data.library_id))
      .exec()
      .await?;

    let library_db_size = match fs::metadata(library_data.library_path.as_str()) {
      Ok(metadata) => metadata.len(),
      Err(_) => 0,
    };

    println!("{:?}", library_statistics);

    let statistics = Statistics {
      library_db_size: library_db_size.to_string(),
      ..Statistics::default()
    };

    let library_local_id = match library {
      Some(library) => library.id,
      None => library_data.library_id,
    };

    db.library_statistics()
      .upsert(library_id::equals(library_local_id))
      .create(
        library_id::set(library_local_id),
        vec![library_db_size::set(statistics.library_db_size.clone())],
      )
      .update(vec![
        total_file_count::set(statistics.total_file_count.clone()),
        total_bytes_used::set(statistics.total_bytes_used.clone()),
        total_bytes_capacity::set(statistics.total_bytes_capacity.clone()),
        total_bytes_free::set(statistics.total_bytes_free.clone()),
        total_unique_bytes::set(statistics.total_unique_bytes.clone()),
        preview_media_bytes::set(statistics.preview_media_bytes.clone()),
        library_db_size::set(statistics.library_db_size.clone()),
      ])
      .exec()
      .await?;

    Ok(statistics)
  }
}
