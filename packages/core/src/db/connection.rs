use crate::state::{self};
use anyhow::Result;
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use sea_orm::{Database, DatabaseConnection};

pub static DB: OnceCell<DatabaseConnection> = OnceCell::new();

pub async fn db() -> Result<&'static DatabaseConnection, String> {
    if DB.get().is_none() {
        let config = state::client::get();

        let current_library = config
            .libraries
            .iter()
            .find(|l| l.library_id == config.current_library_id)
            .unwrap();

        let path = current_library.library_path.clone();

        let db = Database::connect(format!("sqlite://{}", &path))
            .await
            .unwrap();

        DB.set(db).unwrap_or_default();

        Ok(DB.get().unwrap())
    } else {
        Ok(DB.get().unwrap())
    }
}

pub async fn init(db_url: &str) -> Result<(), sqlx::Error> {
    // establish connection, this is only used to create the db if missing
    // replace in future
    let mut connection = Connection::open(&db_url).unwrap();

    // migrate db
    mod embedded_primary {
        use refinery::embed_migrations;
        embed_migrations!("src/db/migrations");
    }

    embedded_primary::migrations::runner()
        .run(&mut connection)
        .unwrap();

    connection.close().unwrap();
    Ok(())
}
