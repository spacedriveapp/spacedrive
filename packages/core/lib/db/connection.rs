use crate::APP_CONFIG;
use anyhow::Result;
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use sea_orm::{Database, DatabaseConnection, DbErr};
// use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
// use sqlx::{ConnectOptions, Connection};
// use std::str::FromStr;

pub async fn get_connection() -> Result<DatabaseConnection, DbErr> {
    let config = &APP_CONFIG.get().unwrap();

    let db_url = format!("{}{}", "sqlite://", config.primary_db.to_str().unwrap());

    let db = Database::connect(&db_url).await?;

    Ok(db)
}

pub static DB_INSTANCE: OnceCell<DatabaseConnection> = OnceCell::new();
pub async fn db_instance() -> Result<&'static DatabaseConnection, String> {
    if DB_INSTANCE.get().is_none() {
        let db = get_connection().await.map_err(|e| e.to_string())?;
        DB_INSTANCE.set(db).unwrap_or_default();
        Ok(DB_INSTANCE.get().unwrap())
    } else {
        Ok(DB_INSTANCE.get().unwrap())
    }
}

pub async fn create_primary_db() -> Result<(), sqlx::Error> {
    let config = &APP_CONFIG.get().unwrap();

    let db_url = config.primary_db.to_str().unwrap();
    // establish connection, this is only used to create the db if missing
    // replace in future
    let mut connection = Connection::open(db_url).unwrap();

    println!("Primary database initialized: {}", &db_url);

    // migrate db
    mod embedded_primary {
        use refinery::embed_migrations;
        embed_migrations!("lib/db/migrations/primary");
    }

    embedded_primary::migrations::runner()
        .run(&mut connection)
        .unwrap();

    // close and exit cause we don't need this connection anymore
    connection.close().unwrap();
    Ok(())
}
