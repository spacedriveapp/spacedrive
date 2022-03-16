use crate::state;
use crate::{prisma, prisma::PrismaClient};
use anyhow::Result;
use once_cell::sync::OnceCell;
use thiserror::Error;
pub mod migrate;

pub static DB: OnceCell<PrismaClient> = OnceCell::new();

#[derive(Error, Debug)]
pub enum DatabaseError {
	#[error("Failed to connect to database")]
	MissingConnection,
	#[error("Unable find current_library in the client config")]
	MalformedConfig,
}

pub async fn get() -> Result<&'static PrismaClient, DatabaseError> {
	if DB.get().is_none() {
		let config = state::client::get();

		let current_library = config
			.libraries
			.iter()
			.find(|l| l.library_uuid == config.current_library_uuid)
			.ok_or(DatabaseError::MalformedConfig)?;

		let path = current_library.library_path.clone();
		// TODO: Error handling when brendan adds it to prisma-client-rust

		let client = prisma::new_client_with_url(&format!("file:{}", &path)).await;
		DB.set(client).unwrap_or_default();

		Ok(DB.get().ok_or(DatabaseError::MissingConnection)?)
	} else {
		Ok(DB.get().ok_or(DatabaseError::MissingConnection)?)
	}
}
