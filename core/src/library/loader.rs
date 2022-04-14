use anyhow::Result;
use thiserror::Error;
use uuid::Uuid;

use crate::state::client::LibraryState;
use crate::{
  db::migrate,
  prisma::{Library, LibraryData},
  state,
};
use crate::{prisma, Core};

pub static LIBRARY_DB_NAME: &str = "library.db";
pub static DEFAULT_NAME: &str = "My Library";

#[derive(Error, Debug)]
pub enum LibraryError {
  #[error("Database error")]
  DatabaseError(#[from] prisma::QueryError),
}

pub async fn get(core: &Core) -> Result<LibraryData, LibraryError> {
  let config = state::client::get();
  let db = &core.database;

  let library_state = config.get_current_library();

  println!("{:?}", library_state);

  // get library from db
  let library = match db
    .library()
    .find_unique(Library::pub_id().equals(library_state.library_uuid.clone()))
    .exec()
    .await?
  {
    Some(library) => Ok(library),
    None => {
      // update config library state to offline
      // config.libraries

      Err(anyhow::anyhow!("library_not_found"))
    }
  };

  Ok(library.unwrap())
}

pub async fn load(library_path: &str, library_id: &str) -> Result<()> {
  let mut config = state::client::get();

  println!("Initializing library: {} {}", &library_id, library_path);

  if config.current_library_uuid != library_id {
    config.current_library_uuid = library_id.to_string();
    config.save();
  }
  // create connection with library database & run migrations
  migrate::run_migrations(&library_path).await?;
  // if doesn't exist, mark as offline
  Ok(())
}

pub async fn create(core: &Core, name: Option<String>) -> Result<()> {
  let mut config = state::client::get();

  let uuid = Uuid::new_v4().to_string();

  println!("Creating library {:?}, UUID: {:?}", name, uuid);

  let library_state = LibraryState {
    library_uuid: uuid.clone(),
    library_path: format!("{}/{}", config.data_path, LIBRARY_DB_NAME),
    ..LibraryState::default()
  };

  migrate::run_migrations(&library_state.library_path).await?;

  config.libraries.push(library_state);

  config.current_library_uuid = uuid;

  config.save();

  let db = &core.database;

  let _library = db
    .library()
    .create(
      Library::pub_id().set(config.current_library_uuid),
      Library::name().set(name.unwrap_or(DEFAULT_NAME.into())),
      vec![],
    )
    .exec()
    .await;

  Ok(())
}
