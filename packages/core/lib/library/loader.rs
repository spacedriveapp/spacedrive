use anyhow::Result;
use sea_orm::ActiveModelTrait;
use sea_orm::Set;
use sea_orm::{entity::*, query::*};
use uuid::Uuid;

use crate::state::client::LibraryState;
use crate::{
    db::connection::{db, init},
    db::entity::library,
    state,
};

pub static LIBRARY_DB_NAME: &str = "library.db";
pub static DEFAULT_NAME: &str = "My Library";

pub async fn get() -> Result<library::Model> {
    let config = state::client::get();
    let db = db().await.unwrap();

    let library_state = config.get_current_library();

    println!("{:?}", library_state);

    // get library from db
    let library = match library::Entity::find()
        .filter(library::Column::Uuid.eq(library_state.library_id.clone()))
        .one(db)
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

    if config.current_library_id != library_id {
        config.current_library_id = library_id.to_string();
        config.save();
    }
    // create connection with library database & run migrations
    init(&library_path).await?;
    // if doesn't exist, mark as offline
    Ok(())
}

pub async fn create(name: Option<String>) -> Result<()> {
    let mut config = state::client::get();

    let uuid = Uuid::new_v4().to_string();

    println!("Creating library {:?}, UUID: {:?}", name, uuid);

    let library_state = LibraryState {
        library_id: uuid.clone(),
        library_path: format!("{}/{}", config.data_path, LIBRARY_DB_NAME),
        ..LibraryState::default()
    };

    init(&library_state.library_path).await?;

    config.libraries.push(library_state);

    config.current_library_id = uuid;

    config.save();

    let db = db().await.unwrap();

    let library = library::ActiveModel {
        uuid: Set(config.current_library_id),
        name: Set(String::from(name.unwrap_or(String::from(DEFAULT_NAME)))),
        ..Default::default()
    };

    library.save(db).await.unwrap();
    Ok(())
}
