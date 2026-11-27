//! # Mount filesystem interface
//!
//! `service::mounts::vfs` is the one shape every mount frontend serves: a
//! read-only tree of directories and files addressed by a path relative to
//! the mount's root. FUSE and the localhost NFS server only translate kernel
//! calls into [`MountFs`] calls, and every backend (the daemon's own sources,
//! a thin client talking to a daemon over the network) only answers them.
//! Keeping the frontends ignorant of where listings and bytes come from is
//! what lets `sd mount` reuse the daemon's FUSE code without a library.
//!
//! Paths are relative and made of plain components; the frontends check every
//! name they are handed, so an implementation never sees `..` or a separator
//! inside one name. Bytes come back as a [`ByteProvider`], so a backend can
//! put the block cache in front of a slow transport without the frontend
//! knowing.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(fs: std::sync::Arc<dyn sd_core::service::mounts::vfs::MountFs>) {
//! use std::path::Path;
//!
//! for entry in fs.list(Path::new("")).await.unwrap() {
//! 	let attr = fs.stat(&Path::new("").join(&entry.name)).await.unwrap();
//! 	tracing::info!(name = ?entry.name, size = attr.size, "listed");
//! }
//! # }
//! ```

use crate::ops::grants::GrantError;
use crate::service::mounts::provider::{ByteError, ByteProvider, ByteTarget};
use async_trait::async_trait;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::broadcast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
	File,
	Directory,
}

/// What a frontend reports for one path.
#[derive(Debug, Clone)]
pub struct NodeAttr {
	pub kind: NodeKind,
	/// Zero for directories.
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub accessed: Option<SystemTime>,
	pub created: Option<SystemTime>,
	/// How long the kernel may trust this answer without asking again.
	pub valid_for: Duration,
}

impl NodeAttr {
	pub fn directory(valid_for: Duration) -> Self {
		Self {
			kind: NodeKind::Directory,
			size: 0,
			modified: None,
			accessed: None,
			created: None,
			valid_for,
		}
	}

	pub fn is_dir(&self) -> bool {
		self.kind == NodeKind::Directory
	}
}

/// One child of a listed directory.
#[derive(Debug, Clone)]
pub struct DirEntry {
	pub name: OsString,
	pub kind: NodeKind,
}

/// A file opened for reading: where its bytes come from and how to ask.
pub struct OpenedFile {
	pub provider: Arc<dyn ByteProvider>,
	pub target: ByteTarget,
}

impl OpenedFile {
	/// Bytes `[offset, offset + size)`, short only at the end of the file.
	pub async fn read(&self, offset: u64, size: u32) -> Result<Vec<u8>, MountError> {
		let end = offset.saturating_add(size as u64);
		let mut out = Vec::with_capacity(size as usize);
		let mut cursor = offset;
		while cursor < end {
			let want = (end - cursor).min(self.provider.max_read());
			let chunk = self
				.provider
				.read_range(&self.target, cursor..cursor + want)
				.await?;
			if chunk.is_empty() {
				break;
			}
			// A cached provider answers at most to the end of one block, so only
			// an empty chunk means the end of the file. Kernels fill a short
			// reply with zeroes, so stopping early would corrupt the read.
			cursor += chunk.len() as u64;
			out.extend_from_slice(&chunk);
		}
		Ok(out)
	}
}

/// Why a request against a mount failed, in the terms a kernel needs.
#[derive(Debug, Clone)]
pub enum MountError {
	NotFound,
	/// The caller may not have this, and retrying will not help.
	Denied(String),
	/// The bytes are not reachable now: a detached drive, a peer offline.
	Unavailable(String),
	/// A file was asked to act as a directory, or the other way round.
	NotADirectory,
	IsADirectory,
}

impl std::fmt::Display for MountError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NotFound => write!(f, "not found"),
			Self::Denied(reason) => write!(f, "denied: {reason}"),
			Self::Unavailable(reason) => write!(f, "unavailable: {reason}"),
			Self::NotADirectory => write!(f, "not a directory"),
			Self::IsADirectory => write!(f, "is a directory"),
		}
	}
}

impl std::error::Error for MountError {}

impl From<ByteError> for MountError {
	fn from(err: ByteError) -> Self {
		match err {
			ByteError::NotFound(_) => Self::NotFound,
			// A cloud placeholder is refused rather than downloaded.
			ByteError::Unsupported(reason) => Self::Denied(reason),
			other => Self::Unavailable(other.to_string()),
		}
	}
}

impl From<GrantError> for MountError {
	fn from(err: GrantError) -> Self {
		match err {
			GrantError::InvalidPath(_) => Self::NotFound,
			GrantError::RootUnavailable(reason) | GrantError::Internal(reason) => {
				Self::Unavailable(reason)
			}
			refusal => Self::Denied(refusal.to_string()),
		}
	}
}

/// A read-only tree a frontend can mount.
#[async_trait]
pub trait MountFs: Send + Sync + 'static {
	/// Attributes of `path`. The empty path is the mount's root, which is
	/// always a directory.
	async fn stat(&self, path: &Path) -> Result<NodeAttr, MountError>;

	/// The children of the directory at `path`, without `.` and `..`.
	async fn list(&self, path: &Path) -> Result<Vec<DirEntry>, MountError>;

	/// The file at `path`, ready to read. Called once per open.
	async fn open(&self, path: &Path) -> Result<OpenedFile, MountError>;

	/// Paths whose cached attributes, listings or bytes are stale, pushed by
	/// backends that learn of changes. Backends without a change feed rely on
	/// each answer's `valid_for`.
	fn changes(&self) -> Option<broadcast::Receiver<PathBuf>> {
		None
	}
}

/// A name a frontend may pass on: exactly one plain component.
pub fn plain(name: &std::ffi::OsStr) -> bool {
	let mut components = Path::new(name).components();
	matches!(
		(components.next(), components.next()),
		(Some(Component::Normal(_)), None)
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::ffi::OsStr;

	#[test]
	fn only_plain_names_pass() {
		assert!(plain(OsStr::new("notes.md")));
		assert!(!plain(OsStr::new("..")));
		assert!(!plain(OsStr::new(".")));
		assert!(!plain(OsStr::new("a/b")));
		assert!(!plain(OsStr::new("")));
	}
}
