use crate::state;
use crate::{prisma, prisma::PrismaClient};
use anyhow::Result;
use thiserror::Error;
pub mod migrate;

#[derive(Error, Debug)]
pub enum DatabaseError {
	#[error("Failed to connect to database")]
	MissingConnection,
	#[error("Unable find current_library in the client config")]
	MalformedConfig,
}

pub async fn create_connection() -> Result<PrismaClient, DatabaseError> {
	let config = state::client::get();

	let current_library = config
		.libraries
		.iter()
		.find(|l| l.library_uuid == config.current_library_uuid)
		.ok_or(DatabaseError::MalformedConfig)?;

	let path = current_library.library_path.clone();
	// TODO: Error handling when brendan adds it to prisma-client-rust

	let client = prisma::new_client_with_url(&format!("file:{}", &path)).await;
	Ok(client)
}
