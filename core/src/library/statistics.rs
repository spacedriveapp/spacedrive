use crate::prisma::library_statistics;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize, TS, Clone)]
#[ts(export)]
pub struct Statistics {
  total_file_count: i32,
  total_bytes_used: String,
  total_bytes_capacity: String,
  total_bytes_free: String,
  total_unique_bytes: String,
  preview_media_bytes: String,
  library_db_size: String,
}

impl Into<Statistics> for library_statistics::Data {
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

// impl Statistics {
//   pub async fn recalculate(ctx: &CoreContext) -> Result<(), LibraryError> {
//     let config = client::get();
//     let db = &ctx.database;

//     let library_data = config.get_current_library();

//     let library_statistics_db = match db
//       .library_statistics()
//       .find_unique(library_statistics::id::equals(library_data.library_id))
//       .exec()
//       .await?
//     {
//       Some(library_statistics_db) => library_statistics_db.into(),
//       // create the default values if database has no entry
//       None => Statistics::default(),
//     };

//     Ok(())
//   }
// }
