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

		if std::env::var("SD_ACCEPT_DATA_LOSS")
			.map(|v| v == "true")
			.unwrap_or(false)
		{
			builder = builder.accept_data_loss();
		}

		if std::env::var("SD_FORCE_RESET_DB")
			.map(|v| v == "true")
			.unwrap_or(false)
		{
			builder = builder.force_reset();
		}

		let res = builder.await;

		match res {
			Ok(_) => {}
			Err(e @ DbPushError::PossibleDataLoss(_)) => {
				eprintln!("Pushing Prisma schema may result in data loss. Use `SD_ACCEPT_DATA_LOSS=true` to force it.");
				Err(e)?;
			}
			Err(e) => Err(e)?,
		}
	}

	#[cfg(not(debug_assertions))]
	client._migrate_deploy().await?;

	Ok(client)
}

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

#[derive(Error, Debug)]
#[error("Missing field {0}")]
pub struct MissingFieldError(&'static str);

pub trait OptionalField: Sized {
	type Out;

	fn transform(self) -> Option<Self::Out>;
}

impl<T> OptionalField for Option<T> {
	type Out = T;

	fn transform(self) -> Option<T> {
		self
	}
}
impl<'a, T> OptionalField for &'a Option<T> {
	type Out = &'a T;

	fn transform(self) -> Option<Self::Out> {
		self.as_ref()
	}
}

pub fn maybe_missing<T: OptionalField>(
	data: T,
	field: &'static str,
) -> Result<T::Out, MissingFieldError> {
	data.transform().ok_or(MissingFieldError(field))
}
