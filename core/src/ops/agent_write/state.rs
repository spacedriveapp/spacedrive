//! A path's state, and whether a precondition still holds for it.

use std::{
	io::Read,
	path::{Path, PathBuf},
};

use super::{PathKind, PathState, Precondition, WriteError};
use crate::{context::CoreContext, ops::indexing::metadata::refuse_dataless_blocking};

/// What a path is on disk now, with the hash a precondition compares.
pub(super) struct Disk {
	pub kind: Option<PathKind>,
	pub size: Option<u64>,
	pub content_hash: Option<String>,
	pub metadata: Option<std::fs::Metadata>,
}

/// Read `path` from disk. Blocking: hashes the file's bytes, or lists the
/// directory.
pub(super) fn disk(path: &Path) -> Result<Disk, WriteError> {
	let metadata = match std::fs::symlink_metadata(path) {
		Ok(metadata) => metadata,
		Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
			return Ok(Disk {
				kind: None,
				size: None,
				content_hash: None,
				metadata: None,
			})
		}
		Err(error) => return Err(WriteError::Io(format!("stat: {error}"))),
	};
	let file_type = metadata.file_type();
	let (kind, content_hash) = if file_type.is_dir() {
		(PathKind::Directory, Some(listing_hash(path)?))
	} else if file_type.is_file() {
		let hash = if refuse_dataless_blocking(path).is_ok() {
			Some(file_hash(path)?)
		} else {
			None
		};
		(PathKind::File, hash)
	} else if file_type.is_symlink() {
		let target = std::fs::read_link(path)
			.map_err(|error| WriteError::Io(format!("read link: {error}")))?;
		(
			PathKind::Symlink,
			Some(
				blake3::hash(target.as_os_str().as_encoded_bytes())
					.to_hex()
					.to_string(),
			),
		)
	} else {
		(PathKind::Other, None)
	};
	Ok(Disk {
		kind: Some(kind),
		size: Some(if file_type.is_dir() {
			0
		} else {
			metadata.len()
		}),
		content_hash,
		metadata: Some(metadata),
	})
}

fn file_hash(path: &Path) -> Result<String, WriteError> {
	let mut file =
		std::fs::File::open(path).map_err(|error| WriteError::Io(format!("open: {error}")))?;
	let mut hasher = blake3::Hasher::new();
	let mut buffer = vec![0u8; 256 * 1024];
	loop {
		let read = file
			.read(&mut buffer)
			.map_err(|error| WriteError::Io(format!("read: {error}")))?;
		if read == 0 {
			break;
		}
		hasher.update(&buffer[..read]);
	}
	Ok(hasher.finalize().to_hex().to_string())
}

/// A directory's hash covers each child's name, kind, size and modification
/// time, sorted by name, so it moves when a child is added, removed, renamed
/// or rewritten, and not when the directory is merely read.
fn listing_hash(path: &Path) -> Result<String, WriteError> {
	let mut children: Vec<(Vec<u8>, String)> = std::fs::read_dir(path)
		.map_err(|error| WriteError::Io(format!("list: {error}")))?
		.filter_map(Result::ok)
		.map(|child| {
			let name = child.file_name().as_encoded_bytes().to_vec();
			let line = match child.path().symlink_metadata() {
				Ok(meta) => format!(
					"{}:{}:{}",
					if meta.is_dir() {
						"d"
					} else if meta.is_symlink() {
						"l"
					} else {
						"f"
					},
					if meta.is_dir() { 0 } else { meta.len() },
					meta.modified()
						.ok()
						.and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
						.map(|at| at.as_nanos())
						.unwrap_or(0)
				),
				Err(_) => "gone".to_string(),
			};
			(name, line)
		})
		.collect();
	children.sort();
	let mut hasher = blake3::Hasher::new();
	for (name, line) in children {
		hasher.update(&name);
		hasher.update(b"\0");
		hasher.update(line.as_bytes());
		hasher.update(b"\n");
	}
	Ok(hasher.finalize().to_hex().to_string())
}

/// The revision of the store that ingests `path`, as `store_id:value`.
pub(super) async fn revision(context: &CoreContext, path: &Path) -> Option<String> {
	let index = context.volume_index();
	let source = index.source_id_for(path)?;
	let db = index.read_store(source).await?;
	let revision = db.revision().await.ok()?;
	Some(format!("{}:{}", revision.store_id.simple(), revision.value))
}

/// `path` relative to the granted directory, with `/` separators.
pub(super) fn shown(root: &Path, path: &Path) -> String {
	path.strip_prefix(root)
		.unwrap_or(path)
		.components()
		.map(|component| component.as_os_str().to_string_lossy().into_owned())
		.collect::<Vec<_>>()
		.join("/")
}

/// Read `path`'s whole state.
pub(super) async fn read(
	context: &CoreContext,
	root: &Path,
	path: &Path,
) -> Result<(PathState, Option<std::fs::Metadata>), WriteError> {
	let owned: PathBuf = path.to_path_buf();
	let disk = tokio::task::spawn_blocking(move || disk(&owned))
		.await
		.map_err(|error| WriteError::Io(format!("state: {error}")))??;
	let state = PathState {
		path: shown(root, path),
		kind: disk.kind,
		size: disk.size,
		content_hash: disk.content_hash,
		revision: revision(context, path).await,
	};
	Ok((state, disk.metadata))
}

/// Refuse unless `expect` names something and all of it matches `current`.
pub(super) fn check(expect: &Precondition, current: &PathState) -> Result<(), WriteError> {
	if expect.is_empty() {
		return Err(WriteError::PreconditionRequired(current.path.clone()));
	}
	let hash_holds = expect.content_hash.as_ref().is_none_or(|wanted| {
		current
			.content_hash
			.as_ref()
			.is_some_and(|now| now.eq_ignore_ascii_case(wanted.trim()))
	});
	let revision_holds = expect
		.revision
		.as_ref()
		.is_none_or(|wanted| current.revision.as_deref() == Some(wanted.trim()));
	if current.exists() && hash_holds && revision_holds {
		return Ok(());
	}
	Err(stale(current.clone()))
}

pub(super) fn stale(current: PathState) -> WriteError {
	WriteError::Stale {
		path: current.path.clone(),
		current: Box::new(current),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn state(hash: Option<&str>, revision: Option<&str>) -> PathState {
		PathState {
			path: "a.txt".to_string(),
			kind: Some(PathKind::File),
			size: Some(1),
			content_hash: hash.map(str::to_string),
			revision: revision.map(str::to_string),
		}
	}

	fn expect(hash: Option<&str>, revision: Option<&str>) -> Precondition {
		Precondition {
			content_hash: hash.map(str::to_string),
			revision: revision.map(str::to_string),
		}
	}

	#[test]
	fn every_named_field_must_match() {
		let now = state(Some("abc"), Some("s:4"));
		assert!(check(&expect(Some("ABC"), None), &now).is_ok());
		assert!(check(&expect(None, Some("s:4")), &now).is_ok());
		assert!(check(&expect(Some("abc"), Some("s:4")), &now).is_ok());
		assert!(matches!(
			check(&expect(Some("abd"), Some("s:4")), &now),
			Err(WriteError::Stale { .. })
		));
		assert!(matches!(
			check(&expect(Some("abc"), Some("s:5")), &now),
			Err(WriteError::Stale { .. })
		));
		assert!(matches!(
			check(&expect(None, None), &now),
			Err(WriteError::PreconditionRequired(_))
		));
	}

	#[test]
	fn a_placeholder_or_a_missing_path_is_stale_for_a_hash() {
		assert!(check(&expect(Some("abc"), None), &state(None, Some("s:1"))).is_err());
		let mut gone = state(None, Some("s:1"));
		gone.kind = None;
		assert!(check(&expect(None, Some("s:1")), &gone).is_err());
	}

	#[test]
	fn a_directory_hash_moves_with_its_children() {
		let dir = tempfile::tempdir().expect("tempdir");
		let before = disk(dir.path()).expect("disk").content_hash;
		assert_eq!(disk(dir.path()).expect("disk").content_hash, before);
		std::fs::write(dir.path().join("child"), b"x").expect("write");
		assert_ne!(disk(dir.path()).expect("disk").content_hash, before);
	}
}
