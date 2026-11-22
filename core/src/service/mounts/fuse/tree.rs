//! What a FUSE mount shows, kept apart from the kernel protocol.
//!
//! The filesystem in `fs.rs` only knows inodes and replies. Everything it
//! shows comes through [`MountTree`]: which directories sit at the top of the
//! mount, which arena lists a path, and which byte provider serves a file.
//! [`LibraryTree`] answers from the daemon's own sources and peer replicas,
//! the same way the WebDAV and SMB shares do, so the mount never shows a
//! source the shares would not. Tests stand in their own tree to mount a
//! simulated peer source without a second device.

use crate::context::CoreContext;
use crate::ops::grants::{authorize, GrantCap, GrantError};
use crate::ops::indexing::arena::Arena;
use crate::ops::indexing::volume_index::SourceStatus;
use crate::service::mounts::peer;
use crate::service::mounts::provider::{
	provider_for, remote_share_name, share_name, ByteError, ByteProvider, MountTarget,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;

/// One source as a directory of the mount.
#[derive(Debug, Clone)]
pub struct MountRoot {
	/// The directory name at the top of the mount. Unused when the mount
	/// shows this root's contents directly.
	pub name: String,
	pub source_id: Uuid,
	/// The path in the source's arena that this directory lists.
	pub path: PathBuf,
	/// The bytes live on another device, so the kernel may keep what the
	/// mount answers for longer.
	pub remote: bool,
}

/// What the top directory of a mount lists.
#[derive(Debug, Clone)]
pub enum Layout {
	/// One directory per source, named as the shares name them.
	Shares(Vec<MountRoot>),
	/// The contents of one directory, for a mount bound to a grant.
	Single(MountRoot),
}

/// Why a request against the mount failed, in the terms the kernel needs.
#[derive(Debug)]
pub enum MountError {
	NotFound,
	/// The caller may not have this, and retrying will not help.
	Denied(String),
	/// The bytes are not reachable now: a detached drive, a peer offline.
	Unavailable(String),
}

impl std::fmt::Display for MountError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NotFound => write!(f, "not found"),
			Self::Denied(reason) => write!(f, "denied: {reason}"),
			Self::Unavailable(reason) => write!(f, "unavailable: {reason}"),
		}
	}
}

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

#[async_trait]
pub trait MountTree: Send + Sync + 'static {
	async fn layout(&self) -> Layout;

	/// The arena that lists `abs`, or `None` when the source is gone.
	async fn arena(&self, source: Uuid, abs: &Path) -> Option<Arc<TokioRwLock<Arena>>>;

	/// Checked before a directory is listed.
	async fn may_list(&self, _source: Uuid, _abs: &Path) -> Result<(), MountError> {
		Ok(())
	}

	/// A provider for the bytes of the file at `abs`, one per open.
	async fn open(&self, source: Uuid, abs: &Path) -> Result<Arc<dyn ByteProvider>, MountError>;
}

/// A grant a mount is bound to: its token, checked again on every listing
/// and open, and the directory it lends.
#[derive(Clone)]
pub struct BoundGrant {
	pub token: String,
	pub root: MountRoot,
}

/// The daemon's sources and peer replicas, optionally narrowed to a grant.
pub struct LibraryTree {
	context: Arc<CoreContext>,
	grant: Option<BoundGrant>,
}

impl LibraryTree {
	pub fn new(context: Arc<CoreContext>, grant: Option<BoundGrant>) -> Self {
		Self { context, grant }
	}

	fn local_source(&self, source: Uuid) -> Option<SourceStatus> {
		self.context
			.volume_index()
			.sources()
			.into_iter()
			.find(|status| status.id == source)
	}

	/// The path the grant allows for `abs`, recorded in the grant's audit
	/// log under `op`. Without a grant every path is allowed as it is.
	async fn authorized(&self, op: &str, abs: &Path) -> Result<PathBuf, MountError> {
		let Some(grant) = &self.grant else {
			return Ok(abs.to_path_buf());
		};
		let scope = authorize(&self.context, &grant.token, op, abs, GrantCap::Read).await?;
		Ok(scope.path)
	}
}

#[async_trait]
impl MountTree for LibraryTree {
	async fn layout(&self) -> Layout {
		if let Some(grant) = &self.grant {
			return Layout::Single(grant.root.clone());
		}
		let mut roots: Vec<MountRoot> = self
			.context
			.volume_index()
			.sources()
			.into_iter()
			.map(|source| MountRoot {
				name: share_name(&source.root, source.id),
				source_id: source.id,
				path: source.root,
				remote: false,
			})
			.collect();
		for share in peer::remote_shares().await {
			roots.push(MountRoot {
				name: remote_share_name(&share),
				source_id: share.info.id,
				path: share.info.root.clone(),
				remote: true,
			});
		}
		roots.sort_by(|a, b| a.name.cmp(&b.name));
		Layout::Shares(roots)
	}

	async fn arena(&self, source: Uuid, abs: &Path) -> Option<Arc<TokioRwLock<Arena>>> {
		if let Some(status) = self.local_source(source) {
			let index = self.context.volume_index();
			index.ensure_restored(&status.root).await;
			return Some(index.resolve_index(abs));
		}
		peer::remote_share(source)
			.await
			.map(|share| share.index.clone())
	}

	async fn may_list(&self, _source: Uuid, abs: &Path) -> Result<(), MountError> {
		self.authorized("fuse.list", abs).await.map(drop)
	}

	async fn open(&self, source: Uuid, abs: &Path) -> Result<Arc<dyn ByteProvider>, MountError> {
		let path = self.authorized("fuse.open", abs).await?;
		let target = match self.local_source(source) {
			Some(status) => MountTarget::Local(status, path),
			None => {
				let share = peer::remote_share(source)
					.await
					.ok_or(MountError::NotFound)?;
				MountTarget::Remote(share, path)
			}
		};
		Ok(provider_for(&self.context, &target).await?)
	}
}
