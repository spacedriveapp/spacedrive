//! Inputs for organizing a folder into subfolders and flattening one.

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::domain::SdPath;

/// Move the files under a folder into subfolders named by a rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileOrganizeInput {
	pub scope: SdPath,
	pub rule: OrganizeRule,
	/// Take the files beneath the folder at any depth, not only those
	/// directly under it. Every file ends up directly under its new
	/// subfolder.
	#[serde(default)]
	pub recursive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OrganizeRule {
	ByDate {
		field: OrganizeDateField,
		granularity: Granularity,
	},
	/// The content kind the indexer assigned: Images, Videos, Documents.
	ByKind,
	/// The extension, lowercased; files without one go under "No extension".
	ByExtension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum OrganizeDateField {
	Modified,
	Created,
	/// The capture time the store holds for a photo or video, or the
	/// modification time where it holds none.
	Captured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum Granularity {
	/// `2024`
	Year,
	/// `2024-05`
	YearMonth,
	/// `2024-05-06`
	YearMonthDay,
}

/// Move every file beneath a folder to the folder itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct FileFlattenInput {
	pub scope: SdPath,
	pub on_conflict: FlattenPolicy,
}

/// What to do with a file whose name is already taken at the root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
pub enum FlattenPolicy {
	/// The deeper file is written under a numbered name.
	KeepBoth,
	/// The deeper file stays where it is.
	Skip,
}
