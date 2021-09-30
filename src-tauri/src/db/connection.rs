use crate::app::config;
use crate::filesystem::file;
use sea_orm::{Database, DatabaseConnection, DbErr, Schema};

pub async fn get_connection() -> Result<DatabaseConnection, DbErr> {
  let config = config::get_config();

  // Database::connect(config.primary_db.to_str().unwrap_or("sqlite::memory:")).await?;
  // Connecting SQLite
  let db_url = format!("{}{}", "sqlite://", config.primary_db.to_str().unwrap());

  let db = Database::connect(&db_url).await?;

  // Derive schema from Entity
  let stmt = Schema::create_table_from_entity(file::Model);

  // Execute create table statement
  let result = db.execute(db.get_database_backend().build(&stmt)).await;

  Ok(db)
}
