use crate::db::connection;
use rusqlite::Connection;
use std::io;

pub async fn migrate_primary() -> io::Result<()> {
  let mut conn = Connection::open(connection::get_primary_db_url()).unwrap();

  println!("Running migrations");
  mod embedded_primary {
    use refinery::embed_migrations;
    embed_migrations!("src/db/migrations/primary");
  }

  embedded_primary::migrations::runner()
    .run(&mut conn)
    .unwrap();

  Ok(())
}
