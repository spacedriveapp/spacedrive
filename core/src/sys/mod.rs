mod locations;
mod volumes;

pub use locations::*;
pub use volumes::*;

use thiserror::Error;

use crate::{job, prisma};

#[derive(Error, Debug)]
pub enum SysError {
	#[error("Location error")]
	Location(#[from] LocationError),
	#[error("Error with system volumes")]
	Volume(String),
	#[error("Error from job runner")]
	Job(#[from] job::JobError),
	#[error("Database error")]
	Database(#[from] prisma::QueryError),
}
