use crate::{prisma, sys::SysError};
use thiserror::Error;

mod library_config;
mod library_ctx;
mod library_manager;
mod statistics;

pub use library_config::*;
pub use library_ctx::*;
pub use library_manager::*;
pub use statistics::*;

#[derive(Error, Debug)]
pub enum LibraryError {
	// #[error("Missing library")]
	// LibraryNotFound,
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
	#[error("System error")]
	SysError(#[from] SysError),
}
