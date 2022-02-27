use anyhow::Result;
use sea_orm::ActiveModelTrait;
use sea_orm::Set;
use uuid::Uuid;

use crate::{
    db::{connection::db, entity::library},
    state,
    state::client::LibraryState,
};

pub async fn init_library() -> Result<()> {
    let mut client_config = state::client::get()?;

    if client_config.libraries.len() == 0 {
        // create default library
        let uuid = Uuid::new_v4().to_string();

        let library = LibraryState {
            library_id: uuid.clone(),
            library_path: format!("{}/library.db", client_config.data_path),
        };

        client_config.libraries.push(library);
        client_config.save();
    }

    Ok(())
}

// this should also take care of calling the connection module to create the library db before saving
pub async fn add_library_to_db(name: Option<String>) {
    let db = db().await.unwrap();

    let library = library::ActiveModel {
        uuid: Set(Uuid::new_v4().to_string()),
        name: Set(String::from(name.unwrap_or(String::from("My Library")))),
        ..Default::default()
    };

    library.save(db).await.unwrap();
}
