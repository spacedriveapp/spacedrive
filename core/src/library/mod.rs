pub mod loader;
pub mod statistics;

use thiserror::Error;

use crate::{prisma, sys::SysError};

#[derive(Error, Debug)]
pub enum LibraryError {
	#[error("Missing library")]
	LibraryNotFound,
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
	#[error("System error")]
	SysError(#[from] SysError),
}
