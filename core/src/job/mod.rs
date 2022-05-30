use std::fmt::Debug;
use thiserror::Error;

use crate::prisma;

mod jobs;
mod worker;

pub use jobs::*;
pub use worker::*;

#[derive(Error, Debug)]
pub enum JobError {
	#[error("Failed to create job (job_id {job_id:?})")]
	CreateFailure { job_id: String },
	#[error("Database error")]
	DatabaseError(#[from] prisma::QueryError),
}
