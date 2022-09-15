use crate::prisma::{self, PrismaClient};
use enumflags2::BitFlags;
use include_dir::{include_dir, Dir};
use migration_core::{
	commands::apply_migrations,
	json_rpc::types::ApplyMigrationsInput,
	migration_connector::{ConnectorError, ConnectorParams},
};
use prisma_client_rust::NewClientError;
use quaint::prelude::*;
use sql_migration_connector::SqlMigrationConnector;
use std::path::Path;
use thiserror::Error;
use tokio::fs::{create_dir, remove_dir_all};
use tracing::debug;

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/prisma/migrations");

/// MigrationError represents an error that occurring while opening a initialising and running migrations on the database.
#[derive(Error, Debug)]
pub enum MigrationError {
	#[error("An error occurred while initialising a new database connection: {0}")]
	NewClient(#[from] Box<NewClientError>),
	#[error("The temporary file path for the database migrations is invalid.")]
	InvalidDirectory,
	#[error("An error occurred creating the temporary directory for the migrations: {0}")]
	CreateDir(std::io::Error),
	#[error("An error occurred extracting the migrations to the temporary directory: {0}")]
	ExtractMigrations(std::io::Error),
	#[error("An error occurred creating the database connection for migrations: {0}")]
	Quiant(#[from] quaint::error::Error),
	#[error("An error occurred running the migrations: {0}")]
	Connector(#[from] ConnectorError),
	#[error("An error occurred removing the temporary directory for the migrations: {0}")]
	RemoveDir(std::io::Error),
}

/// load_and_migrate will load the database from the given path and migrate it to the latest version of the schema.
pub async fn load_and_migrate(
	base_path: &Path,
	db_url: &str,
) -> Result<PrismaClient, MigrationError> {
	let client = prisma::new_client_with_url(db_url)
		.await
		.map_err(Box::new)?;
	let temp_migrations_dir = base_path.join("./migrations_temp");
	let migrations_directory_path = temp_migrations_dir
		.to_str()
		.ok_or(MigrationError::InvalidDirectory)?
		.to_string();

	if temp_migrations_dir.exists() {
		remove_dir_all(&migrations_directory_path)
			.await
			.map_err(MigrationError::RemoveDir)?;
	}

	create_dir(&temp_migrations_dir)
		.await
		.map_err(MigrationError::CreateDir)?;
	MIGRATIONS_DIR
		.extract(&temp_migrations_dir)
		.map_err(MigrationError::ExtractMigrations)?;

	let mut connector = match &ConnectionInfo::from_url(db_url)? {
		ConnectionInfo::Sqlite { .. } => SqlMigrationConnector::new_sqlite(),
		ConnectionInfo::InMemorySqlite { .. } => unreachable!(), // This is how it is in the Prisma Rust tests
	};
	connector.set_params(ConnectorParams {
		connection_string: db_url.to_string(),
		preview_features: BitFlags::empty(),
		shadow_database_connection_string: None,
	})?;

	let output = apply_migrations(
		ApplyMigrationsInput {
			migrations_directory_path,
		},
		&mut connector,
	)
	.await?;

	remove_dir_all(temp_migrations_dir)
		.await
		.map_err(MigrationError::RemoveDir)?;

	for migration in output.applied_migration_names {
		debug!("Applied migration '{}'", migration);
	}

	Ok(client)
}
