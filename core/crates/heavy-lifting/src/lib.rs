#![warn(
	clippy::all,
	clippy::pedantic,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::complexity,
	clippy::nursery,
	clippy::unwrap_used,
	unused_qualifications,
	rust_2018_idioms,
	trivial_casts,
	trivial_numeric_casts,
	unused_allocation,
	clippy::unnecessary_cast,
	clippy::cast_lossless,
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::dbg_macro,
	clippy::deprecated_cfg_attr,
	clippy::separated_literal_suffix,
	deprecated
)]
#![forbid(deprecated_in_future)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

use sd_task_system::TaskSystemError;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub mod file_identifier;
pub mod indexer;
pub mod job_system;
pub mod utils;

use file_identifier::{FileIdentifierError, NonCriticalFileIdentifierError};
use indexer::{IndexerError, NonCriticalIndexerError};

pub use job_system::{
	job::{IntoJob, JobBuilder, JobContext, JobName, JobOutput, JobOutputData, ProgressUpdate},
	JobId, JobSystem,
};

#[derive(Error, Debug)]
pub enum Error {
	#[error(transparent)]
	Indexer(#[from] IndexerError),
	#[error(transparent)]
	FileIdentifier(#[from] FileIdentifierError),

	#[error(transparent)]
	TaskSystem(#[from] TaskSystemError),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::Indexer(e) => e.into(),
			Error::FileIdentifier(e) => e.into(),
			Error::TaskSystem(e) => {
				Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
			}
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type)]
pub enum NonCriticalJobError {
	// TODO: Add variants as needed
	#[error(transparent)]
	Indexer(#[from] NonCriticalIndexerError),
	#[error(transparent)]
	FileIdentifier(#[from] NonCriticalFileIdentifierError),
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum LocationScanState {
	Pending = 0,
	Indexed = 1,
	FilesIdentified = 2,
	Completed = 3,
}
