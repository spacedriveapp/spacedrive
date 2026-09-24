//! Inputs for writing an archive and extracting one.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{domain::SdPath, ops::files::merge::MergeConflictPolicy};

/// Write one archive holding the sources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileArchiveInput {
	pub sources: Vec<SdPath>,
	/// The archive file to write. It must not exist.
	pub destination: SdPath,
	pub format: ArchiveFormat,
	/// Trash the sources once the archive is complete.
	#[serde(default)]
	pub remove_sources: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum ArchiveFormat {
	Zip,
	TarZstd,
}

impl ArchiveFormat {
	pub fn extension(self) -> &'static str {
		match self {
			Self::Zip => "zip",
			Self::TarZstd => "tar.zst",
		}
	}

	/// The format an archive's name says it is in.
	pub fn of_name(name: &str) -> Option<Self> {
		let lower = name.to_lowercase();
		if lower.ends_with(".zip") {
			Some(Self::Zip)
		} else if lower.ends_with(".tar.zst") || lower.ends_with(".tzst") {
			Some(Self::TarZstd)
		} else {
			None
		}
	}
}

/// Extract an archive into a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileExtractInput {
	pub archive: SdPath,
	/// An existing folder the entries are written into.
	pub destination: SdPath,
	pub on_conflict: MergeConflictPolicy,
	/// Leading path components to drop from every entry.
	#[serde(default)]
	pub strip_components: u32,
}

impl FileArchiveInput {
	pub fn validate(&self) -> Result<(), Vec<String>> {
		if self.sources.is_empty() {
			return Err(vec!["name at least one source".to_string()]);
		}
		Ok(())
	}
}
