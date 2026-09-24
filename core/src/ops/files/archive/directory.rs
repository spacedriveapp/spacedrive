//! An archive's own directory: what it holds, before anything is written.
//!
//! A zip carries a central directory, read without decompressing a byte. A
//! tar has no directory, so its headers are read in one pass through the
//! zstd stream, which costs the decompression and no writes.

use std::{
	io,
	path::{Path, PathBuf},
};

use super::input::ArchiveFormat;

/// One entry of an archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
	/// Its position, which an extract resumes from.
	pub index: usize,
	/// The path as the archive spells it.
	pub name: String,
	pub size: u64,
	pub is_dir: bool,
	pub modified_ms: Option<i64>,
}

impl ArchiveEntry {
	/// The path the entry lands at beneath a destination, with leading
	/// components stripped; `None` where the entry would escape, or where
	/// stripping leaves nothing.
	pub fn landing(&self, strip: u32) -> Option<PathBuf> {
		let normalized = self.name.replace('\\', "/");
		let mut components = Vec::new();
		for part in normalized.split('/') {
			match part {
				"" | "." => continue,
				".." => return None,
				part if part.contains('\0') => return None,
				part => components.push(part),
			}
		}
		if normalized.starts_with('/') && !components.is_empty() {
			// An absolute name is read relative to the destination.
		}
		let kept: Vec<&str> = components.into_iter().skip(strip as usize).collect();
		if kept.is_empty() {
			return None;
		}
		Some(kept.iter().collect())
	}
}

/// Read the archive's directory, in entry order.
pub fn read(path: &Path, format: ArchiveFormat) -> io::Result<Vec<ArchiveEntry>> {
	match format {
		ArchiveFormat::Zip => read_zip(path),
		ArchiveFormat::TarZstd => read_tar_zstd(path),
	}
}

fn read_zip(path: &Path) -> io::Result<Vec<ArchiveEntry>> {
	let file = std::fs::File::open(path)?;
	let mut archive = zip::ZipArchive::new(file).map_err(io::Error::other)?;
	let mut entries = Vec::with_capacity(archive.len());
	for index in 0..archive.len() {
		let entry = archive.by_index_raw(index).map_err(io::Error::other)?;
		entries.push(ArchiveEntry {
			index,
			name: entry.name().to_string(),
			size: entry.size(),
			is_dir: entry.is_dir(),
			modified_ms: entry.last_modified().and_then(zip_time_ms),
		});
	}
	Ok(entries)
}

fn zip_time_ms(time: zip::DateTime) -> Option<i64> {
	let date = chrono::NaiveDate::from_ymd_opt(
		i32::from(time.year()),
		u32::from(time.month()),
		u32::from(time.day()),
	)?;
	let when = date.and_hms_opt(
		u32::from(time.hour()),
		u32::from(time.minute()),
		u32::from(time.second()),
	)?;
	Some(when.and_utc().timestamp_millis())
}

fn read_tar_zstd(path: &Path) -> io::Result<Vec<ArchiveEntry>> {
	let file = std::fs::File::open(path)?;
	let decoder = zstd::stream::read::Decoder::new(file)?;
	let mut archive = tar::Archive::new(decoder);
	let mut entries = Vec::new();
	for (index, entry) in archive.entries()?.enumerate() {
		let entry = entry?;
		let header = entry.header();
		entries.push(ArchiveEntry {
			index,
			name: entry.path()?.to_string_lossy().into_owned(),
			size: header.size()?,
			is_dir: header.entry_type().is_dir(),
			modified_ms: header.mtime().ok().map(|seconds| seconds as i64 * 1000),
		});
	}
	Ok(entries)
}

#[cfg(test)]
mod tests {
	use super::*;

	fn entry(name: &str) -> ArchiveEntry {
		ArchiveEntry {
			index: 0,
			name: name.to_string(),
			size: 0,
			is_dir: false,
			modified_ms: None,
		}
	}

	#[test]
	fn an_entry_lands_beneath_the_destination_or_not_at_all() {
		assert_eq!(entry("a/b.txt").landing(0), Some(PathBuf::from("a/b.txt")));
		assert_eq!(entry("a/b.txt").landing(1), Some(PathBuf::from("b.txt")));
		assert_eq!(entry("a/b.txt").landing(2), None);
		assert_eq!(entry("../x").landing(0), None);
		assert_eq!(entry("a/../../x").landing(0), None);
		assert_eq!(
			entry("/etc/passwd").landing(0),
			Some(PathBuf::from("etc/passwd"))
		);
		assert_eq!(entry("./a//b").landing(0), Some(PathBuf::from("a/b")));
	}
}
