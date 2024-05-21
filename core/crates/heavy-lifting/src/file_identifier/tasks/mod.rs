use sd_core_prisma_helpers::file_path_for_file_identifier;

use sd_file_ext::kind::ObjectKind;

use serde::{Deserialize, Serialize};

pub mod extract_file_metadata;
pub mod object_processor;

pub use extract_file_metadata::ExtractFileMetadataTask;
pub use object_processor::ObjectProcessorTask;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct IdentifiedFile {
	pub(super) file_path: file_path_for_file_identifier::Data,
	pub(super) cas_id: Option<String>,
	pub(super) kind: ObjectKind,
}
