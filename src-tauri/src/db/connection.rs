use crate::app::config;
use anyhow::{Context, Result};
use rusqlite::Connection;
use sea_orm::{Database, DatabaseConnection, DbErr};
// use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
// use sqlx::{ConnectOptions, Connection};
// use std::str::FromStr;

pub async fn get_connection() -> Result<DatabaseConnection, DbErr> {
  let config = config::get_config();

  let db_url = format!("{}{}", "sqlite://", config.primary_db.to_str().unwrap());

  let db = Database::connect(&db_url).await?;

  Ok(db)
}

pub async fn create_primary_db() -> Result<(), sqlx::Error> {
  let config = config::get_config();

  let db_url = config.primary_db.to_str().unwrap();
  // establish connection, this is only used to create the db if missing
  // replace in future
  let mut connection = Connection::open(db_url).unwrap();

  println!("Primary database initialized: {}", &db_url);

  // migrate db
  mod embedded_primary {
    use refinery::embed_migrations;
    embed_migrations!("src/db/migrations/primary");
  }

  embedded_primary::migrations::runner()
    .run(&mut connection)
    .unwrap();

  // close and exit cause we don't need this connection anymore
  connection.close().unwrap();
  Ok(())
}
