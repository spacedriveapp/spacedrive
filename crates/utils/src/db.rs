use prisma_client_rust::{
	migrations::{DbPushError, MigrateDeployError},
	NewClientError,
};
use sd_prisma::prisma::PrismaClient;
use thiserror::Error;

/// `[MigrationError]` represents an error that occurring while opening a initialising and running migrations on the database.
#[derive(Error, Debug)]
pub enum MigrationError {
	#[error("An error occurred while initialising a new database connection: {0}")]
	NewClient(#[from] Box<NewClientError>),
	#[error("An error occurred during migration: {0}")]
	MigrateFailed(#[from] MigrateDeployError),
	#[cfg(debug_assertions)]
	#[error("An error occurred during migration: {0}")]
	DbPushFailed(#[from] DbPushError),
}

/// `[load_and_migrate]` will load the database from the given path and migrate it to the latest version of the schema.
pub async fn load_and_migrate(db_url: &str) -> Result<PrismaClient, MigrationError> {
	let client = PrismaClient::_builder()
		.with_url(db_url.to_string())
		.build()
		.await
		.map_err(Box::new)?;

	client._migrate_deploy().await?;

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

	Ok(client)
}

/// Construct back an inode after storing it in database
#[must_use]
pub const fn inode_from_db(db_inode: &[u8]) -> u64 {
	u64::from_le_bytes([
		db_inode[0],
		db_inode[1],
		db_inode[2],
		db_inode[3],
		db_inode[4],
		db_inode[5],
		db_inode[6],
		db_inode[7],
	])
}

/// Constructs a database representation of an inode
#[must_use]
pub fn inode_to_db(inode: u64) -> Vec<u8> {
	inode.to_le_bytes().to_vec()
}

#[must_use]
pub fn ffmpeg_data_field_to_db(field: i64) -> Vec<u8> {
	field.to_be_bytes().to_vec()
}

#[must_use]
pub const fn ffmpeg_data_field_from_db(field: &[u8]) -> i64 {
	i64::from_be_bytes([
		field[0], field[1], field[2], field[3], field[4], field[5], field[6], field[7],
	])
}

#[must_use]
pub const fn size_in_bytes_from_db(db_size_in_bytes: &[u8]) -> u64 {
	u64::from_be_bytes([
		db_size_in_bytes[0],
		db_size_in_bytes[1],
		db_size_in_bytes[2],
		db_size_in_bytes[3],
		db_size_in_bytes[4],
		db_size_in_bytes[5],
		db_size_in_bytes[6],
		db_size_in_bytes[7],
	])
}

#[must_use]
pub fn size_in_bytes_to_db(size: u64) -> Vec<u8> {
	size.to_be_bytes().to_vec()
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
		Self::with_cause(
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

	fn transform(self) -> Self {
		self
	}
}
impl<'a, T> OptionalField for &'a Option<T> {
	type Out = &'a T;

	fn transform(self) -> Option<Self::Out> {
		self.as_ref()
	}
}

/// If `data` is `Some(t)` returns `Ok(t)`, otherwise returns a `MissingFieldError(field)`
pub fn maybe_missing<T: OptionalField>(
	data: T,
	field: &'static str,
) -> Result<T::Out, MissingFieldError> {
	data.transform().ok_or(MissingFieldError(field))
}
