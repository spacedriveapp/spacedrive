use sd_file_path_helper::FilePathError;
use sd_indexer_rules::IndexerRuleError;
use sd_utils::error::{FileIOError, NonUtf8PathError};

pub mod walker;

#[derive(thiserror::Error, Debug)]
pub enum IndexerError {
	#[error(transparent)]
	FileIO(#[from] FileIOError),
	#[error(transparent)]
	NonUtf8Path(#[from] NonUtf8PathError),
	#[error(transparent)]
	IsoFilePath(#[from] FilePathError),
	#[error(transparent)]
	Rule(#[from] IndexerRuleError),
}
