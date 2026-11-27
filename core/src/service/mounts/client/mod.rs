//! # Thin mount client
//!
//! `service::mounts::client` mounts one granted folder from a Spacedrive node
//! without a library, pairing or a running daemon on this machine: it needs a
//! URL and a grant token, and nothing else. `sd mount <url> --token <token>
//! <dir>` is its command line face.
//!
//! [`RemoteFs`] serves the frontends' [`MountFs`] interface from a
//! [`Transport`], the only part that knows how to reach the node. Bytes go
//! through the same L1/L2 block cache the daemon uses for peer reads, so a
//! file is fetched once in 1 MiB blocks and read ahead when it is read in
//! order. Block keys include each file's size and modified time, so a file
//! that changes on the node is fetched again rather than served stale.
//!
//! Attributes learned from a listing are kept for a short validity period so
//! a long listing costs one round trip instead of one per name. Transports
//! that carry a change feed invalidate cached attributes and kernel caches
//! as soon as the node reports a change.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo() -> anyhow::Result<()> {
//! use sd_core::service::mounts::client;
//!
//! let fs = client::connect("http://127.0.0.1:7765", "sdg_...", None).await?;
//! let entries = sd_core::service::mounts::vfs::MountFs::list(&*fs, std::path::Path::new("")).await?;
//! # Ok(())
//! # }
//! ```

pub mod mcp;

use crate::infra::source_dirs::SourceDirs;
use crate::service::mounts::cache::{BlockCache, CachedProvider};
use crate::service::mounts::provider::{
	ByteError, ByteProvider, ByteStat, ByteTarget, ProviderClass,
};
use crate::service::mounts::vfs::{DirEntry, MountError, MountFs, NodeAttr, NodeKind, OpenedFile};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::broadcast;
use uuid::Uuid;

/// How long the kernel and this client trust an answer from the node when
/// no change feed says otherwise.
const VALID_FOR: Duration = Duration::from_secs(1);

/// Default ceiling for the client's block cache on disk.
pub const DEFAULT_CACHE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// One path as the node reports it.
#[derive(Debug, Clone)]
pub struct RemoteNode {
	/// `None` for what a mount does not show: symlinks, devices, sockets.
	pub kind: Option<NodeKind>,
	pub size: u64,
	pub modified: Option<SystemTime>,
	pub created: Option<SystemTime>,
	pub accessed: Option<SystemTime>,
}

/// How a client reaches the node that holds the granted folder. Paths are
/// relative to the grant's folder, `/`-separated, and `""` names the folder.
#[async_trait]
pub trait Transport: Send + Sync + 'static {
	async fn stat(&self, path: &str) -> Result<RemoteNode, MountError>;

	/// Every child of the directory at `path`, with its attributes.
	async fn list(&self, path: &str) -> Result<Vec<(String, RemoteNode)>, MountError>;

	/// Up to `len` bytes from `offset`; fewer only at the end of the file or
	/// when the transport caps one request.
	async fn read(&self, path: &str, offset: u64, len: u64) -> Result<Bytes, MountError>;

	/// Largest read one request answers.
	fn max_read(&self) -> u64;

	/// Paths the node reports as changed, when the transport carries a feed.
	fn changes(&self) -> Option<broadcast::Receiver<PathBuf>> {
		None
	}

	/// Release what the node holds for this client, such as a session.
	async fn close(&self) {}
}

/// Mount `url` with `token`, picking the transport the URL speaks. The block
/// cache lives under `cache_dir`, or the user's cache directory.
pub async fn connect(
	url: &str,
	token: &str,
	cache_dir: Option<PathBuf>,
) -> Result<Arc<RemoteFs>, MountError> {
	let transport = Arc::new(mcp::McpTransport::connect(url, token).await?);
	let cache_dir = match cache_dir {
		Some(dir) => dir,
		None => default_cache_dir()?,
	};
	let dirs = SourceDirs::new(cache_dir)
		.map_err(|err| MountError::Unavailable(format!("block cache: {err}")))?;
	let cache = Arc::new(BlockCache::new(dirs, DEFAULT_CACHE_BYTES));
	cache.restore().await;
	// One cache namespace per node URL and grant, so two mounts never read
	// each other's blocks and a remount finds its own.
	let source = Uuid::new_v5(&Uuid::NAMESPACE_URL, format!("{url}#{token}").as_bytes());
	Ok(Arc::new(RemoteFs::new(transport, cache, source)))
}

fn default_cache_dir() -> Result<PathBuf, MountError> {
	dirs::cache_dir()
		.map(|dir| dir.join("spacedrive").join("mounts"))
		.ok_or_else(|| MountError::Unavailable("no cache directory for this user".to_string()))
}

/// A granted folder on another node, served as a [`MountFs`].
pub struct RemoteFs {
	transport: Arc<dyn Transport>,
	cache: Arc<BlockCache>,
	source: Uuid,
	attrs: Arc<Mutex<HashMap<PathBuf, (NodeAttr, Instant)>>>,
	changes: Option<broadcast::Sender<PathBuf>>,
}

impl RemoteFs {
	pub fn new(transport: Arc<dyn Transport>, cache: Arc<BlockCache>, source: Uuid) -> Self {
		let attrs: Arc<Mutex<HashMap<PathBuf, (NodeAttr, Instant)>>> = Arc::default();
		// Forget cached attributes before the frontend hears of a change, so
		// the kernel's next question gets a fresh answer.
		let changes = transport.changes().map(|mut feed| {
			let (relay, _) = broadcast::channel(1024);
			let (attrs, out) = (attrs.clone(), relay.clone());
			tokio::spawn(async move {
				use broadcast::error::RecvError;
				loop {
					match feed.recv().await {
						Ok(path) => {
							if let Ok(mut attrs) = attrs.lock() {
								attrs.remove(&path);
								if let Some(parent) = path.parent() {
									attrs.remove(parent);
								}
							}
							let _ = out.send(path);
						}
						Err(RecvError::Lagged(_)) => {
							if let Ok(mut attrs) = attrs.lock() {
								attrs.clear();
							}
						}
						Err(RecvError::Closed) => break,
					}
				}
			});
			relay
		});
		Self {
			transport,
			cache,
			source,
			attrs,
			changes,
		}
	}

	/// Release the node's session for this client.
	pub async fn close(&self) {
		self.transport.close().await;
	}

	fn cached(&self, path: &Path) -> Option<NodeAttr> {
		let attrs = self.attrs.lock().ok()?;
		let (attr, at) = attrs.get(path)?;
		(at.elapsed() < attr.valid_for).then(|| attr.clone())
	}

	fn remember(&self, path: PathBuf, attr: NodeAttr) {
		if let Ok(mut attrs) = self.attrs.lock() {
			// Bounded by what the kernel has asked about recently; expired
			// answers are dropped as they are found.
			if attrs.len() > 100_000 {
				attrs.retain(|_, (attr, at)| at.elapsed() < attr.valid_for);
			}
			attrs.insert(path, (attr, Instant::now()));
		}
	}

	async fn fresh_stat(&self, path: &Path) -> Result<NodeAttr, MountError> {
		let node = self.transport.stat(&wire_path(path)?).await?;
		let attr = attr(&node).ok_or(MountError::NotFound)?;
		self.remember(path.to_path_buf(), attr.clone());
		Ok(attr)
	}
}

/// A mount-relative path as the transport spells it.
fn wire_path(path: &Path) -> Result<String, MountError> {
	let mut out = String::new();
	for component in path.components() {
		let std::path::Component::Normal(name) = component else {
			return Err(MountError::NotFound);
		};
		let name = name.to_str().ok_or(MountError::NotFound)?;
		if !out.is_empty() {
			out.push('/');
		}
		out.push_str(name);
	}
	Ok(out)
}

fn attr(node: &RemoteNode) -> Option<NodeAttr> {
	Some(NodeAttr {
		kind: node.kind?,
		size: if node.kind? == NodeKind::Directory {
			0
		} else {
			node.size
		},
		modified: node.modified,
		accessed: node.accessed,
		created: node.created,
		valid_for: VALID_FOR,
	})
}

#[async_trait]
impl MountFs for RemoteFs {
	async fn stat(&self, path: &Path) -> Result<NodeAttr, MountError> {
		if let Some(attr) = self.cached(path) {
			return Ok(attr);
		}
		self.fresh_stat(path).await
	}

	async fn list(&self, path: &Path) -> Result<Vec<DirEntry>, MountError> {
		let children = self.transport.list(&wire_path(path)?).await?;
		let mut entries = Vec::with_capacity(children.len());
		for (name, node) in children {
			let Some(attr) = attr(&node) else {
				continue;
			};
			entries.push(DirEntry {
				name: name.clone().into(),
				kind: attr.kind,
			});
			self.remember(path.join(&name), attr);
		}
		Ok(entries)
	}

	async fn open(&self, path: &Path) -> Result<OpenedFile, MountError> {
		// An open always asks the node, so the cache key below names the
		// bytes the node has now.
		let attr = self.fresh_stat(path).await?;
		if attr.is_dir() {
			return Err(MountError::IsADirectory);
		}
		let file = Arc::new(RemoteFile {
			transport: self.transport.clone(),
			path: wire_path(path)?,
			stat: ByteStat {
				size: attr.size,
				// A file without a time still needs a stable cache key.
				modified: Some(attr.modified.unwrap_or(SystemTime::UNIX_EPOCH)),
			},
		});
		Ok(OpenedFile {
			provider: Arc::new(CachedProvider::new(file, self.cache.clone())),
			target: ByteTarget {
				source_id: self.source,
				path: path.to_path_buf(),
			},
		})
	}

	fn changes(&self) -> Option<broadcast::Receiver<PathBuf>> {
		self.changes.as_ref().map(broadcast::Sender::subscribe)
	}
}

/// One open file's bytes through the transport.
struct RemoteFile {
	transport: Arc<dyn Transport>,
	path: String,
	stat: ByteStat,
}

impl From<MountError> for ByteError {
	fn from(err: MountError) -> Self {
		match err {
			MountError::NotFound => ByteError::NotFound(String::new()),
			MountError::Denied(reason) => ByteError::Unsupported(reason),
			other => ByteError::Io(other.to_string()),
		}
	}
}

#[async_trait]
impl ByteProvider for RemoteFile {
	async fn stat(&self, _target: &ByteTarget) -> Result<ByteStat, ByteError> {
		Ok(self.stat)
	}

	async fn read_range(
		&self,
		_target: &ByteTarget,
		range: Range<u64>,
	) -> Result<Bytes, ByteError> {
		let end = range.end.min(self.stat.size);
		let mut out = BytesMut::new();
		let mut cursor = range.start;
		while cursor < end {
			let chunk = self
				.transport
				.read(&self.path, cursor, end - cursor)
				.await?;
			if chunk.is_empty() {
				break;
			}
			cursor += chunk.len() as u64;
			out.extend_from_slice(&chunk);
		}
		Ok(out.freeze())
	}

	fn class(&self) -> ProviderClass {
		ProviderClass::Peer
	}

	fn max_read(&self) -> u64 {
		self.transport.max_read()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn wire_paths_are_slash_separated_and_relative() {
		assert_eq!(wire_path(Path::new("")).unwrap(), "");
		assert_eq!(wire_path(Path::new("a/b.txt")).unwrap(), "a/b.txt");
		assert!(wire_path(Path::new("../x")).is_err());
		assert!(wire_path(Path::new("/etc")).is_err());
	}
}
