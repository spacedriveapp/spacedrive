use crate::prisma::{self, PrismaClient};
use prisma_client_rust::QueryError;
use prisma_client_rust::{migrations::*, NewClientError};
use sd_crypto::keys::keymanager::StoredKey;
use thiserror::Error;

/// MigrationError represents an error that occurring while opening a initialising and running migrations on the database.
#[derive(Error, Debug)]
pub enum MigrationError {
	#[error("An error occurred while initialising a new database connection: {0}")]
	NewClient(#[from] Box<NewClientError>),
	#[cfg(debug_assertions)]
	#[error("An error occurred during migration: {0}")]
	MigrateFailed(#[from] DbPushError),
	#[cfg(not(debug_assertions))]
	#[error("An error occurred during migration: {0}")]
	MigrateFailed(#[from] MigrateDeployError),
}

/// load_and_migrate will load the database from the given path and migrate it to the latest version of the schema.
pub async fn load_and_migrate(db_url: &str) -> Result<PrismaClient, MigrationError> {
	let client = prisma::new_client_with_url(db_url)
		.await
		.map_err(Box::new)?;

	#[cfg(debug_assertions)]
	{
		let mut builder = client._db_push();

		if std::env::var("SD_FORCE_RESET_DB")
			.map(|v| v == "true")
			.unwrap_or(false)
		{
			builder = builder.accept_data_loss().force_reset();
		}

		builder.await?;
	}

	#[cfg(not(debug_assertions))]
	client._migrate_deploy().await?;

	Ok(client)
}

/// This writes a `StoredKey` to prisma
/// If the key is marked as memory-only, it is skipped
pub async fn write_storedkey_to_db(db: &PrismaClient, key: &StoredKey) -> Result<(), QueryError> {
	if !key.memory_only {
		db.key()
			.create(
				key.uuid.to_string(),
				key.algorithm.serialize().to_vec(),
				key.hashing_algorithm.serialize().to_vec(),
				key.content_salt.to_vec(),
				key.master_key.to_vec(),
				key.master_key_nonce.to_vec(),
				key.key_nonce.to_vec(),
				key.key.to_vec(),
				vec![],
			)
			.exec()
			.await?;
	}

	Ok(())
}
