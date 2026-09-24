//! What a filesystem accepts as a name.
//!
//! Two checks. The portable one refuses what no filesystem takes: an empty
//! name, a path separator, `.` and `..`, and NUL; it runs in `from_input`,
//! so a structurally broken name never reaches a job. The filesystem check
//! knows the target volume: Windows filesystems and SMB shares refuse the
//! characters and device names NTFS does and a trailing dot or space, Apple
//! filesystems refuse a colon, and every filesystem has a length past which
//! a name cannot be written. That check is a validation finding, since it
//! needs the volume registry.

use std::path::Path;

use thiserror::Error;

use crate::volume::types::FileSystem;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NameProblem {
	#[error("the name is empty")]
	Empty,
	#[error("a name cannot contain a path separator")]
	Separator,
	#[error("a name cannot be '.' or '..'")]
	Dot,
	#[error("the name contains a character the filesystem refuses: {0:?}")]
	Character(char),
	#[error("{0} is a name Windows filesystems reserve for a device")]
	Reserved(String),
	#[error("a name cannot end with a space or a period on this filesystem")]
	Ending,
	#[error("the name is longer than the {0} {1} the filesystem allows")]
	TooLong(usize, &'static str),
}

/// Names Windows filesystems reserve for devices, matched on the stem
/// without regard to case.
const RESERVED: &[&str] = &[
	"CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
	"COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Which rules a filesystem applies to names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameRules {
	/// NTFS, ReFS, FAT32 and exFAT, and SMB, whose protocol carries the
	/// same limits whatever sits behind the share.
	Windows,
	/// APFS and HFS+.
	Apple,
	/// Everything else: a separator and NUL are the only forbidden bytes.
	Posix,
}

impl NameRules {
	pub fn of(file_system: &FileSystem) -> Self {
		match file_system {
			FileSystem::NTFS
			| FileSystem::ReFS
			| FileSystem::FAT32
			| FileSystem::ExFAT
			| FileSystem::SMB => Self::Windows,
			FileSystem::APFS | FileSystem::HFSPlus => Self::Apple,
			FileSystem::Ext4
			| FileSystem::Btrfs
			| FileSystem::ZFS
			| FileSystem::NFS
			| FileSystem::Other(_) => match file_system {
				FileSystem::Other(name) if name.eq_ignore_ascii_case("cifs") => Self::Windows,
				FileSystem::Other(name) if name.eq_ignore_ascii_case("smbfs") => Self::Windows,
				FileSystem::Other(name) if name.eq_ignore_ascii_case("msdos") => Self::Windows,
				_ => Self::Posix,
			},
		}
	}

	/// Whether the filesystem matches names without regard to case by
	/// default. A volume can be formatted otherwise, so a rename probes the
	/// directory before trusting this.
	pub fn case_insensitive(file_system: &FileSystem) -> bool {
		matches!(Self::of(file_system), Self::Windows | Self::Apple)
	}
}

/// What no filesystem accepts.
pub fn check_portable(name: &str) -> Result<(), NameProblem> {
	if name.is_empty() {
		return Err(NameProblem::Empty);
	}
	if name.contains('/') || name.contains('\\') {
		return Err(NameProblem::Separator);
	}
	if name == "." || name == ".." {
		return Err(NameProblem::Dot);
	}
	if name.contains('\0') {
		return Err(NameProblem::Character('\0'));
	}
	Ok(())
}

/// Whether the filesystem writes the name, after [`check_portable`].
pub fn check_name(name: &str, rules: NameRules) -> Result<(), NameProblem> {
	check_portable(name)?;
	match rules {
		NameRules::Windows => {
			if let Some(bad) = name.chars().find(|c| {
				matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*') || (*c as u32) < 0x20
			}) {
				return Err(NameProblem::Character(bad));
			}
			let stem = Path::new(name)
				.file_stem()
				.and_then(|stem| stem.to_str())
				.unwrap_or(name);
			if RESERVED
				.iter()
				.any(|reserved| stem.eq_ignore_ascii_case(reserved))
			{
				return Err(NameProblem::Reserved(stem.to_uppercase()));
			}
			if name.ends_with(' ') || name.ends_with('.') {
				return Err(NameProblem::Ending);
			}
			if name.encode_utf16().count() > 255 {
				return Err(NameProblem::TooLong(255, "characters"));
			}
		}
		NameRules::Apple => {
			if name.contains(':') {
				return Err(NameProblem::Character(':'));
			}
			if name.len() > 255 {
				return Err(NameProblem::TooLong(255, "bytes"));
			}
		}
		NameRules::Posix => {
			if name.len() > 255 {
				return Err(NameProblem::TooLong(255, "bytes"));
			}
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_portable_check_refuses_what_no_filesystem_takes() {
		assert_eq!(check_portable(""), Err(NameProblem::Empty));
		assert_eq!(check_portable("a/b"), Err(NameProblem::Separator));
		assert_eq!(check_portable("a\\b"), Err(NameProblem::Separator));
		assert_eq!(check_portable(".."), Err(NameProblem::Dot));
		assert_eq!(check_portable("a\0b"), Err(NameProblem::Character('\0')));
		assert_eq!(check_portable("日本語ファイル.txt"), Ok(()));
	}

	/// Windows rules refuse the NTFS character set, device names and a
	/// trailing dot; Apple rules refuse a colon; POSIX takes both.
	#[test]
	fn each_filesystem_refuses_its_own_names() {
		assert_eq!(
			check_name("a:b", NameRules::Windows),
			Err(NameProblem::Character(':'))
		);
		assert_eq!(
			check_name("con.txt", NameRules::Windows),
			Err(NameProblem::Reserved("CON".into()))
		);
		assert_eq!(
			check_name("done.", NameRules::Windows),
			Err(NameProblem::Ending)
		);
		assert_eq!(
			check_name("a:b", NameRules::Apple),
			Err(NameProblem::Character(':'))
		);
		assert_eq!(check_name("con.txt", NameRules::Apple), Ok(()));
		assert_eq!(check_name("a:b", NameRules::Posix), Ok(()));
		assert_eq!(check_name("a?b", NameRules::Posix), Ok(()));
		assert_eq!(
			check_name(&"x".repeat(256), NameRules::Posix),
			Err(NameProblem::TooLong(255, "bytes"))
		);
		assert_eq!(check_name(&"é".repeat(200), NameRules::Windows), Ok(()));
		assert_eq!(
			check_name(&"é".repeat(200), NameRules::Apple),
			Err(NameProblem::TooLong(255, "bytes"))
		);
	}

	#[test]
	fn rules_follow_the_filesystem() {
		assert_eq!(NameRules::of(&FileSystem::NTFS), NameRules::Windows);
		assert_eq!(NameRules::of(&FileSystem::SMB), NameRules::Windows);
		assert_eq!(
			NameRules::of(&FileSystem::Other("cifs".into())),
			NameRules::Windows
		);
		assert_eq!(NameRules::of(&FileSystem::APFS), NameRules::Apple);
		assert_eq!(NameRules::of(&FileSystem::Ext4), NameRules::Posix);
		assert!(NameRules::case_insensitive(&FileSystem::APFS));
		assert!(!NameRules::case_insensitive(&FileSystem::Ext4));
	}
}
