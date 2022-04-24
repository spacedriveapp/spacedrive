pub mod loader;
pub mod statistics;

use thiserror::Error;

use crate::{prisma, sys::SysError};

#[derive(Error, Debug)]
pub enum LibraryError {
  #[error("Database error")]
  DatabaseError(#[from] prisma::QueryError),
  #[error("System error")]
  SysError(#[from] SysError),
}
