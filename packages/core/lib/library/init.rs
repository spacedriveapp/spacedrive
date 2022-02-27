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
    let db = db().await.unwrap();

    if client_config.libraries.len() == 0 {
        // create default library
        let uuid = Uuid::new_v4().to_string();

        let library = LibraryState {
            library_id: uuid.clone(),
            library_path: format!("{}/primary_library.db", client_config.data_path),
        };

        client_config.libraries.push(library);
        client_config.save();

        let library = library::ActiveModel {
            uuid: Set(uuid),
            name: Set(String::from("My Library")),
            ..Default::default()
        };

        library.save(db).await?;
    }

    Ok(())
}
