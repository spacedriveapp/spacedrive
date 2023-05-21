use crate::prisma::{self, PrismaClient};
use prisma_client_rust::{migrations::*, NewClientError};
use thiserror::Error;
use uuid::Uuid;

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

// /// This writes a `StoredKey` to prisma
// /// If the key is marked as memory-only, it is skipped
// pub async fn write_storedkey_to_db(
// 	db: &PrismaClient,
// 	stored_key: &StoredKey,
// ) -> Result<(), LibraryManagerError> {
// 	if !stored_key.memory_only {
// 		db.key()
// 			.create(
// 				stored_key.uuid.to_string(),
// 				encoding::encode(&stored_key.version)?,
// 				encoding::encode(&stored_key.key_type)?,
// 				encoding::encode(&stored_key.algorithm)?,
// 				encoding::encode(&stored_key.hashing_algorithm)?,
// 				encoding::encode(&stored_key.content_salt)?,
// 				encoding::encode(&stored_key.master_key)?,
// 				encoding::encode(&stored_key.master_key_nonce)?,
// 				encoding::encode(&stored_key.key_nonce)?,
// 				encoding::encode(&stored_key.key)?,
// 				encoding::encode(&stored_key.salt)?,
// 				vec![],
// 			)
// 			.exec()
// 			.await?;
// 	}

// 	Ok(())
// }

/// Combines an iterator of `T` and an iterator of `Option<T>`,
/// removing any `None` values in the process
pub fn chain_optional_iter<T>(
	required: impl IntoIterator<Item = T>,
	optional: impl IntoIterator<Item = Option<T>>,
) -> Vec<T> {
	required
		.into_iter()
		.map(Some)
		.chain(optional)
		.flatten()
		.collect()
}

pub fn uuid_to_bytes(uuid: Uuid) -> Vec<u8> {
	uuid.as_bytes().to_vec()
}
