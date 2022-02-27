use crate::db::{connection::db, entity::library};
use anyhow::{bail, Result};
use sea_orm::{entity::*, DatabaseConnection, QueryFilter};
use strum::Display;

#[derive(Display)]
pub enum InitError {
    LibraryNotFound,
}

pub async fn get_primary_library(db: &DatabaseConnection) -> Result<library::Model> {
    // get library entity by is_primary column, should be unique
    let mut existing_libs = library::Entity::find()
        .filter(library::Column::IsPrimary.eq(true))
        .all(db)
        .await?;

    // return library
    if existing_libs.len() == 0 {
        bail!(InitError::LibraryNotFound.to_string());
    } else {
        Ok(existing_libs.swap_remove(0))
    }
}

pub async fn init_library() -> Result<()> {
    let db = db().await.unwrap();

    let library = get_primary_library(&db).await;
    // if no library create one now
    if library.is_err() {
        let library = library::ActiveModel {
            name: Set("Primary".to_owned()),
            is_primary: Set(true),
            ..Default::default()
        };

        let library = library.save(db).await?;

        println!("created library {:?}", &library);
    } else {
        // println!("library loaded {:?}", library.unwrap());
    };

    Ok(())
}
