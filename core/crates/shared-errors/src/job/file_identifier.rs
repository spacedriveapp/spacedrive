use crate::file_helper::Error as FilePathError;
use crate::job::sub_path;
use crate::library_sync;

use prisma_client_rust::QueryError;
use rspc::ErrorCode;
use sd_prisma::prisma::file_path;
use sd_sync::DevicePubId;
use sd_utils::db::MissingFieldError;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("device not found: <device_pub_id='{0}'")]
	DeviceNotFound(DevicePubId),
	#[error("missing field on database: {0}")]
	MissingField(#[from] MissingFieldError),
	#[error("failed to deserialized stored tasks for job resume: {0}")]
	DeserializeTasks(#[from] rmp_serde::decode::Error),
	#[error("database error: {0}")]
	Database(#[from] QueryError),

	#[error(transparent)]
	FilePathError(#[from] FilePathError),
	#[error(transparent)]
	SubPath(#[from] sub_path::Error),
	#[error(transparent)]
	Sync(#[from] library_sync::Error),
}

impl From<Error> for rspc::Error {
	fn from(e: Error) -> Self {
		match e {
			Error::SubPath(sub_path_err) => sub_path_err.into(),

			_ => Self::with_cause(ErrorCode::InternalServerError, e.to_string(), e),
		}
	}
}

#[derive(thiserror::Error, Debug, Serialize, Deserialize, Type, Clone)]
#[serde(rename_all = "snake_case")]
pub enum NonCriticalFileIdentifierError {
	#[error("failed to extract file metadata: {0}")]
	FailedToExtractFileMetadata(String),
	#[cfg(target_os = "windows")]
	#[error("failed to extract metadata from on-demand file: {0}")]
	FailedToExtractMetadataFromOnDemandFile(String),
	#[error(
		"failed to extract isolated file path data: <file_path_id='{file_path_pub_id}'>: {error}"
	)]
	FailedToExtractIsolatedFilePathData {
		file_path_pub_id: Uuid,
		error: String,
	},
	#[error("file path without is_dir field: <file_path_id='{0}'>")]
	FilePathWithoutIsDirField(file_path::id::Type),
}
