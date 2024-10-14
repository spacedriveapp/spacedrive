#![recursion_limit = "256"]
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

use sd_prisma::prisma::file_path;
use sd_task_system::TaskSystemError;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;

pub mod file_identifier;
pub mod indexer;
pub mod job_system;
pub mod media_processor;
pub mod utils;

use media_processor::ThumbKey;

pub use job_system::{
	job::{
		IntoJob, JobContext, JobEnqueuer, JobName, JobOutput, JobOutputData, OuterContext,
		ProgressUpdate,
	},
	report::Report,
	JobId, JobSystem, JobSystemError,
};

#[derive(Error, Debug)]
pub enum Error {
	#[error(transparent)]
	Indexer(#[from] indexer::Error),
	#[error(transparent)]
	FileIdentifier(#[from] file_identifier::Error),
	#[error(transparent)]
	MediaProcessor(#[from] media_processor::Error),
	#[error("TODO")] //  TODO(matheus-consoli): error handling
	Deleter(String),

	#[error(transparent)]
	TaskSystem(#[from] TaskSystemError),

	#[error(transparent)]
	JobSystem(#[from] JobSystemError),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::Indexer(e) => e.into(),
			Error::FileIdentifier(e) => e.into(),
			Error::MediaProcessor(e) => e.into(),
			Error::TaskSystem(e) => {
				Self::with_cause(rspc::ErrorCode::InternalServerError, e.to_string(), e)
			}
			Error::JobSystem(e) => e.into(),
			Error::Deleter(_) => todo!(),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NonCriticalError {
	// TODO: Add variants as needed
	#[error(transparent)]
	Indexer(#[from] indexer::NonCriticalIndexerError),
	#[error(transparent)]
	FileIdentifier(#[from] file_identifier::NonCriticalFileIdentifierError),
	#[error(transparent)]
	MediaProcessor(#[from] media_processor::NonCriticalMediaProcessorError),
	#[error("TODO")]
	Deleter(String),
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, Eq, PartialEq)]
pub enum LocationScanState {
	Pending = 0,
	Indexed = 1,
	FilesIdentified = 2,
	Completed = 3,
}

#[derive(Debug, Serialize, Type)]
pub enum UpdateEvent {
	NewThumbnail {
		thumb_key: ThumbKey,
	},
	NewIdentifiedObjects {
		file_path_ids: Vec<file_path::id::Type>,
	},
}
