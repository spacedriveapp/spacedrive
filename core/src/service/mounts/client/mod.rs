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

use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::sync::{broadcast, watch};
use tracing::info;
use uuid::Uuid;

use crate::infra::source_dirs::SourceDirs;
use crate::service::mounts::cache::{BlockCache, CachedProvider};
use crate::service::mounts::provider::{
	ByteError, ByteProvider, ByteStat, ByteTarget, ProviderClass,
};
use crate::service::mounts::vfs::{DirEntry, MountError, MountFs, NodeAttr, NodeKind, OpenedFile};

/// How long the kernel and this client trust an answer from the node when
/// no change feed says otherwise.
const VALID_FOR: Duration = Duration::from_secs(1);

/// How often an idle mount asks the node whether its grant still holds.
const GRANT_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// What every transport answers with when the node refuses the token, so
/// the client can tell a grant that ended from any other refusal.
pub const TOKEN_REFUSED: &str = "invalid, expired or revoked grant token";

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
	let dirs = tokio::task::spawn_blocking(move || -> anyhow::Result<SourceDirs> {
		let dirs = SourceDirs::new(cache_dir.clone())?;
		// Blocks are copies of granted files, so only this user may reach
		// them, whatever the umask and however the directory was made before.
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			std::fs::set_permissions(&cache_dir, std::fs::Permissions::from_mode(0o700))?;
		}
		Ok(dirs)
	})
	.await
	.map_err(|err| MountError::Unavailable(err.to_string()))?
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
	changes: broadcast::Sender<PathBuf>,
	/// Set once the node refuses the token; nothing is served after that,
	/// cached blocks included.
	ended: Arc<AtomicBool>,
	ended_tx: watch::Sender<bool>,
	/// Set once the frontend reports the kernel dropped its caches after the
	/// grant ended.
	flushed_tx: watch::Sender<bool>,
}

impl RemoteFs {
	pub fn new(transport: Arc<dyn Transport>, cache: Arc<BlockCache>, source: Uuid) -> Self {
		let attrs: Arc<Mutex<HashMap<PathBuf, (NodeAttr, Instant)>>> = Arc::default();
		// Forget cached attributes before the frontend hears of a change, so
		// the kernel's next question gets a fresh answer.
		let (changes, _) = broadcast::channel(1024);
		if let Some(mut feed) = transport.changes() {
			let (attrs, out) = (attrs.clone(), changes.clone());
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
		}
		Self {
			transport,
			cache,
			source,
			attrs,
			changes,
			ended: Arc::new(AtomicBool::new(false)),
			ended_tx: watch::Sender::new(false),
			flushed_tx: watch::Sender::new(false),
		}
	}

	/// Returns once the node refuses the token: the grant was revoked or
	/// expired. From then on every listing, open and read is refused, and
	/// the kernel is told to drop everything it cached, so a file opened
	/// before cannot be read from memory either. It returns only after every
	/// frontend listening for changes has reported that it did so. Callers
	/// unmount.
	pub async fn grant_ended(&self) {
		let mut ended = self.ended_tx.subscribe();
		loop {
			if *ended.borrow_and_update() {
				let mut flushed = self.flushed_tx.subscribe();
				let _ = flushed.wait_for(|done| *done).await;
				return;
			}
			tokio::select! {
				_ = ended.changed() => {}
				_ = tokio::time::sleep(GRANT_CHECK_INTERVAL) => {
					let _ = self.checked(self.transport.stat("").await);
				}
			}
		}
	}

	fn end(&self) {
		if self.ended.swap(true, Ordering::SeqCst) {
			return;
		}
		info!("the node refused the grant token; refusing everything on this mount");
		if let Ok(mut attrs) = self.attrs.lock() {
			attrs.clear();
		}
		// The root path stands for every path. With no frontend listening
		// there is nothing to flush.
		if self.changes.send(PathBuf::new()).is_err() {
			self.flushed_tx.send_replace(true);
		}
		self.ended_tx.send_replace(true);
	}

	fn refused() -> MountError {
		MountError::Denied(TOKEN_REFUSED.to_string())
	}

	/// Pass `result` on, ending the mount when it says the token is refused.
	fn checked<T>(&self, result: Result<T, MountError>) -> Result<T, MountError> {
		if self.ended.load(Ordering::SeqCst) {
			return Err(Self::refused());
		}
		if matches!(&result, Err(MountError::Denied(reason)) if reason == TOKEN_REFUSED) {
			self.end();
		}
		result
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
		let node = self.checked(self.transport.stat(&wire_path(path)?).await)?;
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
			return self.checked(Ok(attr));
		}
		self.fresh_stat(path).await
	}

	async fn list(&self, path: &Path) -> Result<Vec<DirEntry>, MountError> {
		let children = self.checked(self.transport.list(&wire_path(path)?).await)?;
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
			provider: Arc::new(Gated {
				inner: CachedProvider::new(file, self.cache.clone()),
				ended: self.ended.clone(),
			}),
			target: ByteTarget {
				source_id: self.source,
				path: path.to_path_buf(),
			},
		})
	}

	fn changes(&self) -> Option<broadcast::Receiver<PathBuf>> {
		Some(self.changes.subscribe())
	}

	fn invalidated(&self, path: &Path) {
		if path.as_os_str().is_empty() && self.ended.load(Ordering::SeqCst) {
			self.flushed_tx.send_replace(true);
		}
	}
}

/// Refuses every read once the grant has ended, before the block cache can
/// answer from blocks fetched while it held.
struct Gated {
	inner: CachedProvider,
	ended: Arc<AtomicBool>,
}

#[async_trait]
impl ByteProvider for Gated {
	async fn stat(&self, target: &ByteTarget) -> Result<ByteStat, ByteError> {
		self.inner.stat(target).await
	}

	async fn read_range(&self, target: &ByteTarget, range: Range<u64>) -> Result<Bytes, ByteError> {
		if self.ended.load(Ordering::SeqCst) {
			return Err(ByteError::Unsupported(TOKEN_REFUSED.to_string()));
		}
		self.inner.read_range(target, range).await
	}

	fn class(&self) -> ProviderClass {
		self.inner.class()
	}

	fn max_read(&self) -> u64 {
		self.inner.max_read()
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
