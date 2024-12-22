use sd_task_system::TaskSystemError;

use serde::{Deserialize, Serialize};
use specta::Type;
use thiserror::Error;
pub mod file_identifier;
pub mod indexer;
pub mod media_processor;
pub mod report;
pub mod sub_path;
pub mod system;

use crate::system::JobSystemError;

#[derive(Error, Debug)]
pub enum Error {
	#[error(transparent)]
	Indexer(#[from] indexer::Error),
	#[error(transparent)]
	FileIdentifier(#[from] file_identifier::Error),
	#[error(transparent)]
	MediaProcessor(#[from] media_processor::Error),

	#[error(transparent)]
	TaskSystem(#[from] TaskSystemError),

	#[error(transparent)]
	JobSystem(#[from] JobSystemError),

	#[error(transparent)]
	SubPath(#[from] sub_path::Error),
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
			Error::SubPath(e) => e.into(),
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
}
