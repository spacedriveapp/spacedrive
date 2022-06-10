use crate::prisma::{self, migration, PrismaClient};
use data_encoding::HEXLOWER;
use include_dir::{include_dir, Dir};
use prisma_client_rust::raw;
use ring::digest::{Context, Digest, SHA256};
use std::ffi::OsStr;
use std::io::{self, BufReader, Read};
use std::sync::Arc;
use thiserror::Error;

const INIT_MIGRATION: &str = include_str!("../../prisma/migrations/migration_table/migration.sql");
static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/prisma/migrations");

#[derive(Error, Debug)]
pub enum DatabaseError {
	#[error("Unable to initialize the Prisma client")]
	ClientError(#[from] prisma::NewClientError),
}

pub async fn create_connection(path: &str) -> Result<PrismaClient, DatabaseError> {
	println!("Creating database connection: {:?}", path);
	let client = prisma::new_client_with_url(&format!("file:{}", &path)).await?;

	Ok(client)
}

pub fn sha256_digest<R: Read>(mut reader: R) -> Result<Digest, io::Error> {
	let mut context = Context::new(&SHA256);
	let mut buffer = [0; 1024];
	loop {
		let count = reader.read(&mut buffer)?;
		if count == 0 {
			break;
		}
		context.update(&buffer[..count]);
	}
	Ok(context.finish())
}

pub async fn run_migrations(db: Arc<PrismaClient>) -> Result<(), DatabaseError> {
	match db
		._query_raw::<serde_json::Value>(raw!(
			"SELECT name FROM sqlite_master WHERE type='table' AND name='_migrations'"
		))
		.await
	{
		Ok(data) => {
			if data.len() == 0 {
				// execute migration
				match db._execute_raw(raw!(INIT_MIGRATION)).await {
					Ok(_) => {}
					Err(e) => {
						println!("Failed to create migration table: {}", e);
					}
				};

				let value: Vec<serde_json::Value> = db
					._query_raw(raw!(
						"SELECT name FROM sqlite_master WHERE type='table' AND name='_migrations'"
					))
					.await
					.unwrap();

				#[cfg(debug_assertions)]
				println!("Migration table created: {:?}", value);
			}

			let mut migration_subdirs = MIGRATIONS_DIR
				.dirs()
				.filter(|subdir| {
					subdir
						.path()
						.file_name()
						.map(|name| name != OsStr::new("migration_table"))
						.unwrap_or(false)
				})
				.collect::<Vec<_>>();

			migration_subdirs.sort_by(|a, b| {
				let a_name = a.path().file_name().unwrap().to_str().unwrap();
				let b_name = b.path().file_name().unwrap().to_str().unwrap();

				let a_time = a_name[..14].parse::<i64>().unwrap();
				let b_time = b_name[..14].parse::<i64>().unwrap();

				a_time.cmp(&b_time)
			});

			for subdir in migration_subdirs {
				println!("{:?}", subdir.path());
				let migration_file = subdir
					.get_file(subdir.path().join("./migration.sql"))
					.unwrap();
				let migration_sql = migration_file.contents_utf8().unwrap();

				let digest = sha256_digest(BufReader::new(migration_file.contents())).unwrap();
				// create a lowercase hash from
				let checksum = HEXLOWER.encode(digest.as_ref());
				let name = subdir.path().file_name().unwrap().to_str().unwrap();

				// get existing migration by checksum, if it doesn't exist run the migration
				let existing_migration = db
					.migration()
					.find_unique(migration::checksum::equals(checksum.clone()))
					.exec()
					.await
					.unwrap();

				if existing_migration.is_none() {
					#[cfg(debug_assertions)]
					println!("Running migration: {}", name);

					let steps = migration_sql.split(";").collect::<Vec<&str>>();
					let steps = &steps[0..steps.len() - 1];

					db.migration()
						.create(
							migration::name::set(name.to_string()),
							migration::checksum::set(checksum.clone()),
							vec![],
						)
						.exec()
						.await
						.unwrap();

					for (i, step) in steps.iter().enumerate() {
						match db._execute_raw(raw!(*step)).await {
							Ok(_) => {
								db.migration()
									.find_unique(migration::checksum::equals(checksum.clone()))
									.update(vec![migration::steps_applied::set(i as i32 + 1)])
									.exec()
									.await
									.unwrap();
							}
							Err(e) => {
								println!("Error running migration: {}", name);
								println!("{}", e);
								break;
							}
						}
					}

					#[cfg(debug_assertions)]
					println!("Migration {} recorded successfully", name);
				}
			}
		}
		Err(err) => {
			panic!("Failed to check migration table existence: {:?}", err);
		}
	}

	Ok(())
}
