use crate::db::connection::db_instance;
use crate::db::entity::file;
use anyhow::Result;
use sea_orm::ColumnTrait;
use sea_orm::{EntityTrait, QueryFilter};
use serde::Serialize;

#[derive(Serialize)]
pub struct Directory {
  pub directory: file::Model,
  pub contents: Vec<file::Model>,
}

pub async fn get_dir_with_contents(path: &str) -> Result<Directory, String> {
  let connection = db_instance().await?;

  println!("getting files... {:?}", &path);

  let directories = file::Entity::find()
    .filter(file::Column::Uri.eq(path))
    .all(connection)
    .await
    .map_err(|e| e.to_string())?;

  if directories.is_empty() {
    return Err("directory_not_found".to_owned());
  }

  let directory = &directories[0];

  let files = file::Entity::find()
    .filter(file::Column::ParentId.eq(directory.id))
    .all(connection)
    .await
    .map_err(|e| e.to_string())?;

  Ok(Directory {
    directory: directory.clone(),
    contents: files,
  })
}
