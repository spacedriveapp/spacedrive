use crate::state;
use crate::{prisma, prisma::PrismaClient};
use thiserror::Error;
pub mod migrate;

#[derive(Error, Debug)]
pub enum DatabaseError {
	#[error("Failed to connect to database")]
	MissingConnection,
	#[error("Unable find current_library in the client config")]
	MalformedConfig,
	#[error("Unable to initialize the Prisma client")]
	ClientError(#[from] prisma::NewClientError),
}

pub async fn create_connection() -> Result<PrismaClient, DatabaseError> {
	let config = state::client::get();

	let current_library = config.get_current_library();

	let path = current_library.library_path.clone();

	let client = prisma::new_client_with_url(&format!("file:{}", &path)).await?;

	Ok(client)
}
