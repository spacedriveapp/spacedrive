mod locations;
mod volumes;

pub use locations::*;
pub use volumes::*;

use thiserror::Error;

use crate::{job, prisma};

#[derive(Error, Debug)]
pub enum SysError {
	#[error("Location error")]
	LocationError(#[from] LocationError),
	#[error("Error with system volumes")]
	VolumeError(String),
	#[error("Error from job runner")]
	JobError(#[from] job::JobError),
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
}
