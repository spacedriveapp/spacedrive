//! Output types for create folder operations

use crate::domain::addressing::SdPath;
use crate::infra::job::handle::JobReceipt;
use serde::{Deserialize, Serialize};
use specta::Type;

/// Output from creating a folder
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CreateFolderOutput {
	/// Path to the created folder
	pub folder_path: SdPath,
	/// Job receipt if items were moved into the folder
	#[serde(skip_serializing_if = "Option::is_none")]
	pub job_receipt: Option<JobReceipt>,
}

impl CreateFolderOutput {
	/// Create output for a folder created without moving items
	pub fn without_items(folder_path: SdPath) -> Self {
		Self {
			folder_path,
			job_receipt: None,
		}
	}

	/// Create output for a folder created with items being moved
	pub fn with_items(folder_path: SdPath, job_receipt: JobReceipt) -> Self {
		Self {
			folder_path,
			job_receipt: Some(job_receipt),
		}
	}
}
