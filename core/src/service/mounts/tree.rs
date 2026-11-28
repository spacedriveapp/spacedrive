//! What a daemon-side mount shows: the daemon's sources, as arenas.
//!
//! [`MountTree`] says which directories sit at the top of the mount, which
//! arena lists a path, and which byte provider serves a file.
//! [`LibraryTree`] answers from the daemon's own sources and peer replicas,
//! the same way the WebDAV and SMB shares do, so a mount never shows a source
//! the shares would not. [`ArenaFs`] turns any such tree into the path-based
//! [`MountFs`] the frontends serve. Tests stand in their own tree to mount a
//! simulated peer source without a second device.

use crate::context::CoreContext;
use crate::ops::grants::{authorize, Access, GrantCap};
use crate::ops::indexing::arena::Arena;
use crate::ops::indexing::metadata::EntryMetadata;
use crate::ops::indexing::state::EntryKind;
use crate::ops::indexing::volume_index::SourceStatus;
use crate::service::mounts::peer;
use crate::service::mounts::provider::{
	provider_for, remote_share_name, share_name, ByteProvider, ByteTarget, MountTarget,
};
pub use crate::service::mounts::vfs::MountError;
use crate::service::mounts::vfs::{DirEntry, MountFs, NodeAttr, NodeKind, OpenedFile};
use async_trait::async_trait;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock as TokioRwLock;
use uuid::Uuid;

/// A local arena follows its watcher within about a second.
const LOCAL_TTL: Duration = Duration::from_secs(1);
/// Peer replicas refresh every 30 seconds, so asking sooner learns little.
const REMOTE_TTL: Duration = Duration::from_secs(10);

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
	/// Names the frontend in the grant's audit log, as `fuse.open` or
	/// `nfs.open`.
	frontend: &'static str,
}

impl LibraryTree {
	pub fn new(
		context: Arc<CoreContext>,
		grant: Option<BoundGrant>,
		frontend: &'static str,
	) -> Self {
		Self {
			context,
			grant,
			frontend,
		}
	}

	fn local_source(&self, source: Uuid) -> Option<SourceStatus> {
		self.context
			.volume_index()
			.sources()
			.into_iter()
			.find(|status| status.id == source)
	}

	/// The path the grant allows for `abs`, recorded in the grant's audit
	/// log under `op` prefixed with the frontend. Without a grant every path is allowed as it is.
	async fn authorized(&self, op: &str, abs: &Path) -> Result<PathBuf, MountError> {
		let Some(grant) = &self.grant else {
			return Ok(abs.to_path_buf());
		};
		let op = format!("{}.{op}", self.frontend);
		let access = Access::new(&grant.token, &op, abs, GrantCap::Read);
		Ok(authorize(&self.context, &access).await?.scope.path)
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
		self.authorized("list", abs).await.map(drop)
	}

	async fn open(&self, source: Uuid, abs: &Path) -> Result<Arc<dyn ByteProvider>, MountError> {
		let path = self.authorized("open", abs).await?;
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

/// Where a mount path lands: a path in one source's arena.
#[derive(Debug, Clone)]
struct Located {
	source: Uuid,
	abs: PathBuf,
	/// The source's root, which is a directory whether or not its arena
	/// holds an entry for it.
	is_root: bool,
	remote: bool,
}

impl Located {
	fn ttl(&self) -> Duration {
		if self.remote {
			REMOTE_TTL
		} else {
			LOCAL_TTL
		}
	}
}

/// Any [`MountTree`] served as a path-based [`MountFs`].
pub struct ArenaFs<T: ?Sized> {
	tree: Arc<T>,
}

impl<T: MountTree + ?Sized> ArenaFs<T> {
	pub fn new(tree: Arc<T>) -> Self {
		Self { tree }
	}

	/// The source path behind `path`, or `None` for the top of a mount that
	/// lists sources.
	async fn locate(&self, path: &Path) -> Result<Option<Located>, MountError> {
		let (root, rest) = match self.tree.layout().await {
			Layout::Single(root) => (root, path),
			Layout::Shares(roots) => {
				let mut components = path.components();
				let Some(first) = components.next() else {
					return Ok(None);
				};
				let Component::Normal(first) = first else {
					return Err(MountError::NotFound);
				};
				let root = roots
					.into_iter()
					.find(|root| first == root.name.as_str())
					.ok_or(MountError::NotFound)?;
				(root, components.as_path())
			}
		};
		let is_root = rest.as_os_str().is_empty();
		Ok(Some(Located {
			source: root.source_id,
			// Joining an empty path would add a trailing separator the arena
			// does not know.
			abs: if is_root {
				root.path
			} else {
				root.path.join(rest)
			},
			is_root,
			remote: root.remote,
		}))
	}

	/// The arena's entry for `at`, `None` for a source root without one.
	async fn describe(&self, at: &Located) -> Result<Option<EntryMetadata>, MountError> {
		let arena = self
			.tree
			.arena(at.source, &at.abs)
			.await
			.ok_or(MountError::NotFound)?;
		// Symlinks are left out: the arena keeps no link target, and a target
		// would resolve outside the mount, past any grant.
		let meta = arena
			.read()
			.await
			.get_entry_ref(&at.abs)
			.filter(|meta| meta.kind != EntryKind::Symlink);
		match meta {
			Some(meta) if !at.is_root => Ok(Some(meta)),
			_ if at.is_root => Ok(None),
			_ => Err(MountError::NotFound),
		}
	}
}

fn attr(meta: &EntryMetadata, valid_for: Duration) -> NodeAttr {
	if meta.kind == EntryKind::Directory {
		return NodeAttr {
			modified: meta.modified,
			..NodeAttr::directory(valid_for)
		};
	}
	NodeAttr {
		kind: NodeKind::File,
		size: meta.size,
		modified: meta.modified,
		accessed: meta.accessed,
		created: meta.created,
		valid_for,
	}
}

#[async_trait]
impl<T: MountTree + ?Sized> MountFs for ArenaFs<T> {
	async fn stat(&self, path: &Path) -> Result<NodeAttr, MountError> {
		let Some(at) = self.locate(path).await? else {
			return Ok(NodeAttr::directory(LOCAL_TTL));
		};
		Ok(match self.describe(&at).await? {
			Some(meta) => attr(&meta, at.ttl()),
			None => NodeAttr::directory(at.ttl()),
		})
	}

	async fn list(&self, path: &Path) -> Result<Vec<DirEntry>, MountError> {
		let Some(at) = self.locate(path).await? else {
			let Layout::Shares(roots) = self.tree.layout().await else {
				return Ok(Vec::new());
			};
			return Ok(roots
				.into_iter()
				.map(|root| DirEntry {
					name: OsString::from(root.name),
					kind: NodeKind::Directory,
				})
				.collect());
		};
		if let Some(meta) = self.describe(&at).await? {
			if meta.kind != EntryKind::Directory {
				return Err(MountError::NotADirectory);
			}
		}
		self.tree.may_list(at.source, &at.abs).await?;
		let arena = self
			.tree
			.arena(at.source, &at.abs)
			.await
			.ok_or(MountError::NotFound)?;
		let arena = arena.read().await;
		let mut entries = Vec::new();
		for child in arena.list_directory(&at.abs).unwrap_or_default() {
			let (Some(meta), Some(name)) = (arena.get_entry_ref(&child), child.file_name()) else {
				continue;
			};
			let kind = match meta.kind {
				EntryKind::File => NodeKind::File,
				EntryKind::Directory => NodeKind::Directory,
				EntryKind::Symlink => continue,
			};
			entries.push(DirEntry {
				name: name.to_os_string(),
				kind,
			});
		}
		Ok(entries)
	}

	async fn open(&self, path: &Path) -> Result<OpenedFile, MountError> {
		let Some(at) = self.locate(path).await? else {
			return Err(MountError::IsADirectory);
		};
		match self.describe(&at).await? {
			Some(meta) if meta.kind == EntryKind::File => {}
			_ => return Err(MountError::IsADirectory),
		}
		let provider = self.tree.open(at.source, &at.abs).await?;
		Ok(OpenedFile {
			provider,
			target: ByteTarget {
				source_id: at.source,
				path: at.abs,
			},
		})
	}
}
