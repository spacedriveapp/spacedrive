use prisma_client_rust::{migrations::*, NewClientError};
use sd_prisma::prisma::{self, PrismaClient};
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

pub fn inode_from_db(db_inode: &[u8]) -> u64 {
	u64::from_le_bytes(db_inode.try_into().expect("corrupted inode in database"))
}

pub fn inode_to_db(inode: u64) -> Vec<u8> {
	inode.to_le_bytes().to_vec()
}

#[derive(Error, Debug)]
#[error("Missing field {0}")]
pub struct MissingFieldError(&'static str);

impl MissingFieldError {
	#[must_use]
	pub const fn new(value: &'static str) -> Self {
		Self(value)
	}
}

impl From<MissingFieldError> for rspc::Error {
	fn from(value: MissingFieldError) -> Self {
		rspc::Error::with_cause(
			rspc::ErrorCode::InternalServerError,
			"Missing crucial data in the database".to_string(),
			value,
		)
	}
}

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
