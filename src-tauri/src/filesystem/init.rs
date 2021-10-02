use crate::db::connection;
use crate::db::entity::library;
use sea_orm::entity::*;
use sea_orm::DbErr;
use sea_orm::QueryFilter;

pub async fn init_library() -> Result<(), Box<DbErr>> {
  let db = connection::get_connection().await?;

  let existing_libs = library::Entity::find()
    .filter(library::Column::IsPrimary.eq(true))
    .all(&db)
    .await
    .unwrap();

  if existing_libs.len() == 0 {
    let library = library::ActiveModel {
      name: Set("Primary".to_owned()),
      is_primary: Set(true),
      ..Default::default()
    };

    let library = library.save(&db).await?;

    println!("created library {:?}", library);
  } else {
    let existing_lib = &existing_libs[0];
    println!("library loaded {:?}", existing_lib);
  };

  Ok(())
}
