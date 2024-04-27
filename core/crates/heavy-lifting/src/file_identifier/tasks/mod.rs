use sd_core_prisma_helpers::file_path_for_file_identifier;

use sd_file_ext::kind::ObjectKind;

use serde::{Deserialize, Serialize};

mod extract_file_metadata;
mod object_processor;

pub use extract_file_metadata::{ExtractFileMetadataTask, ExtractFileMetadataTaskOutput};
pub use object_processor::{ObjectProcessorTask, ObjectProcessorTaskMetrics};

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct IdentifiedFile {
	pub(super) file_path: file_path_for_file_identifier::Data,
	pub(super) cas_id: Option<String>,
	pub(super) kind: ObjectKind,
}
