use crate::prisma::{self, migration, PrismaClient};
use data_encoding::HEXLOWER;
use include_dir::{include_dir, Dir};
use prisma_client_rust::{raw, NewClientError};
use ring::digest::{Context, SHA256};
use thiserror::Error;

const INIT_MIGRATION: &str = include_str!("../../prisma/migrations/migration_table/migration.sql");
static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/prisma/migrations");

/// MigrationError represents an error that occurring while opening a initialising and running migrations on the database.
#[derive(Error, Debug)]
pub enum MigrationError {
	#[error("An error occurred while initialising a new database connection")]
	DatabaseInitialization(#[from] NewClientError),
	#[error("An error occurred with the database while applying migrations")]
	DatabaseError(#[from] prisma_client_rust::queries::Error),
	#[error("An error occurred reading the embedded migration files. {0}. Please report to Spacedrive developers!")]
	InvalidEmbeddedMigration(&'static str),
}

/// load_and_migrate will load the database from the given path and migrate it to the latest version of the schema.
pub async fn load_and_migrate(db_url: &str) -> Result<PrismaClient, MigrationError> {
	let client = prisma::new_client_with_url(db_url).await?;

	let migrations_table_missing = client
		._query_raw::<serde_json::Value>(raw!(
			"SELECT name FROM sqlite_master WHERE type='table' AND name='_migrations'"
		))
		.await?
		.is_empty();

	if migrations_table_missing {
		client._execute_raw(raw!(INIT_MIGRATION)).await?;
	}

	let mut migration_directories = MIGRATIONS_DIR
		.dirs()
		.map(|dir| {
			dir.path()
				.file_name()
				.ok_or(MigrationError::InvalidEmbeddedMigration(
					"File has malformed name",
				))
				.and_then(|name| {
					name.to_str()
						.ok_or(MigrationError::InvalidEmbeddedMigration(
							"File name contains malformed characters",
						))
						.map(|name| (name, dir))
				})
		})
		.filter_map(|v| match v {
			Ok((name, _)) if name == "migration_table" => None,
			Ok((name, dir)) => match name[..14].parse::<i64>() {
				Ok(timestamp) => Some(Ok((name, timestamp, dir))),
				Err(_) => Some(Err(MigrationError::InvalidEmbeddedMigration(
					"File name is incorrectly formatted",
				))),
			},
			Err(v) => Some(Err(v)),
		})
		.collect::<Result<Vec<_>, _>>()?;

	// We sort the migrations so they are always applied in the correct order
	migration_directories.sort_by(|(_, a_time, _), (_, b_time, _)| a_time.cmp(b_time));

	for (name, _, dir) in migration_directories {
		let migration_file_raw = dir
			.get_file(dir.path().join("./migration.sql"))
			.ok_or(MigrationError::InvalidEmbeddedMigration(
				"Failed to find 'migration.sql' file in '{}' migration subdirectory",
			))?
			.contents_utf8()
			.ok_or(
				MigrationError::InvalidEmbeddedMigration(
					"Failed to open the contents of 'migration.sql' file in '{}' migration subdirectory",
				)
			)?;

		// Generate SHA256 checksum of migration
		let mut checksum = Context::new(&SHA256);
		checksum.update(migration_file_raw.as_bytes());
		let checksum = HEXLOWER.encode(checksum.finish().as_ref());

		// get existing migration by checksum, if it doesn't exist run the migration
		if client
			.migration()
			.find_unique(migration::checksum::equals(checksum.clone()))
			.exec()
			.await?
			.is_none()
		{
			// Create migration record
			client
				.migration()
				.create(name.to_string(), checksum.clone(), vec![])
				.exec()
				.await?;

			// Split the migrations file up into each individual step and apply them all
			let steps = migration_file_raw.split(';').collect::<Vec<&str>>();
			let steps = &steps[0..steps.len() - 1];
			for (i, step) in steps.iter().enumerate() {
				client._execute_raw(raw!(*step)).await?;
				client
					.migration()
					.find_unique(migration::checksum::equals(checksum.clone()))
					.update(vec![migration::steps_applied::set(i as i32 + 1)])
					.exec()
					.await?;
			}
		}
	}

	Ok(client)
}
