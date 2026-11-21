//! The kernel side of the mount: inode numbers, attributes and replies.
//!
//! fuser calls every method on its session thread and expects no async. Each
//! call becomes a task on the daemon's runtime that owns its reply, so a slow
//! peer read never holds up a listing on the same mount.
//!
//! Inode numbers are a hash of the source and path, so a path keeps its
//! number for as long as the kernel remembers it and a listing can report
//! one without allocating anything. Only looked-up paths are kept, and each
//! is dropped when the kernel forgets it.

use super::tree::{Layout, MountError, MountRoot, MountTree};
use crate::ops::indexing::metadata::EntryMetadata;
use crate::ops::indexing::state::EntryKind;
use crate::service::mounts::provider::{ByteProvider, ByteTarget};
use fuser::{
	BackgroundSession, Config, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags,
	Generation, INodeNo, LockOwner, MountOption, OpenAccMode, OpenFlags, ReplyAttr, ReplyData,
	ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs, Request,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::future::Future;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::runtime::Handle;
use tracing::debug;
use uuid::Uuid;

const ROOT: u64 = 1;
/// A local arena follows its watcher within about a second.
const LOCAL_TTL: Duration = Duration::from_secs(1);
/// Peer replicas refresh every 30 seconds, so asking sooner learns little.
const REMOTE_TTL: Duration = Duration::from_secs(10);
/// Reported as the preferred I/O size, so tools read in large ranges.
const BLOCK_SIZE: u32 = 128 * 1024;

/// Mount `tree` at `mountpoint`, read-only. The mount lasts until the
/// returned session is unmounted or dropped.
pub async fn mount(
	tree: Arc<dyn MountTree>,
	mountpoint: &Path,
) -> std::io::Result<BackgroundSession> {
	let root = match tree.layout().await {
		Layout::Single(root) => Located::source_root(&root),
		Layout::Shares(_) => Located {
			source: None,
			abs: PathBuf::new(),
			is_root: true,
			remote: false,
		},
	};
	let fs = SpacedriveFs {
		inner: Arc::new(Inner {
			tree,
			inodes: Mutex::new(Inodes::new(root)),
			files: Mutex::new(HashMap::new()),
			dirs: Mutex::new(HashMap::new()),
			next_fh: AtomicU64::new(1),
			// Owned by the daemon's user, the only one the kernel lets in
			// without allow_other.
			uid: unsafe { libc::getuid() },
			gid: unsafe { libc::getgid() },
			mounted_at: SystemTime::now(),
		}),
		runtime: Handle::current(),
	};
	let mut config = Config::default();
	config.mount_options = vec![
		MountOption::RO,
		MountOption::NoAtime,
		MountOption::DefaultPermissions,
		MountOption::FSName("spacedrive".to_string()),
		MountOption::Subtype("spacedrive".to_string()),
	];
	let mountpoint = mountpoint.to_path_buf();
	tokio::task::spawn_blocking(move || fuser::spawn_mount(fs, &mountpoint, &config))
		.await
		.map_err(std::io::Error::other)?
}

/// Where an inode points: a path in one source's arena, or the top of a
/// mount that lists sources.
#[derive(Debug, Clone)]
struct Located {
	source: Option<Uuid>,
	abs: PathBuf,
	/// The source's root, which is a directory whether or not its arena
	/// holds an entry for it.
	is_root: bool,
	remote: bool,
}

impl Located {
	fn source_root(root: &MountRoot) -> Self {
		Self {
			source: Some(root.source_id),
			abs: root.path.clone(),
			is_root: true,
			remote: root.remote,
		}
	}

	fn same(&self, other: &Located) -> bool {
		self.source == other.source && self.abs == other.abs
	}

	fn ttl(&self) -> Duration {
		if self.remote {
			REMOTE_TTL
		} else {
			LOCAL_TTL
		}
	}
}

struct Node {
	at: Located,
	lookups: u64,
}

struct Inodes {
	nodes: HashMap<u64, Node>,
}

impl Inodes {
	fn new(root: Located) -> Self {
		let mut nodes = HashMap::new();
		nodes.insert(
			ROOT,
			Node {
				at: root,
				lookups: 1,
			},
		);
		Self { nodes }
	}

	fn get(&self, ino: u64) -> Option<Located> {
		self.nodes.get(&ino).map(|node| node.at.clone())
	}

	/// The number `at` has, or would get if it were looked up now.
	fn number(&self, at: &Located) -> u64 {
		let mut hasher = DefaultHasher::new();
		at.source.hash(&mut hasher);
		at.abs.hash(&mut hasher);
		let mut ino = hasher.finish().max(ROOT + 1);
		while let Some(node) = self.nodes.get(&ino) {
			if node.at.same(at) {
				break;
			}
			ino = ino.wrapping_add(1).max(ROOT + 1);
		}
		ino
	}

	fn remember(&mut self, at: Located) -> u64 {
		if let Some(node) = self.nodes.get_mut(&ROOT) {
			if node.at.same(&at) {
				return ROOT;
			}
		}
		let ino = self.number(&at);
		self.nodes
			.entry(ino)
			.or_insert(Node { at, lookups: 0 })
			.lookups += 1;
		ino
	}

	fn forget(&mut self, ino: u64, count: u64) {
		if ino == ROOT {
			return;
		}
		if let Some(node) = self.nodes.get_mut(&ino) {
			node.lookups = node.lookups.saturating_sub(count);
			if node.lookups == 0 {
				self.nodes.remove(&ino);
			}
		}
	}
}

struct OpenFile {
	provider: Arc<dyn ByteProvider>,
	target: ByteTarget,
}

struct Listed {
	ino: u64,
	kind: FileType,
	name: OsString,
}

struct Inner {
	tree: Arc<dyn MountTree>,
	inodes: Mutex<Inodes>,
	files: Mutex<HashMap<u64, Arc<OpenFile>>>,
	dirs: Mutex<HashMap<u64, Arc<Vec<Listed>>>>,
	next_fh: AtomicU64,
	uid: u32,
	gid: u32,
	mounted_at: SystemTime,
}

struct SpacedriveFs {
	inner: Arc<Inner>,
	runtime: Handle,
}

fn errno(err: &MountError) -> Errno {
	match err {
		MountError::NotFound => Errno::ENOENT,
		MountError::Denied(_) => Errno::EACCES,
		MountError::Unavailable(_) => Errno::EIO,
	}
}

/// A name the kernel may ask for inside a directory: one plain component.
fn plain(name: &OsStr) -> bool {
	let mut components = Path::new(name).components();
	matches!(
		(components.next(), components.next()),
		(Some(Component::Normal(_)), None)
	)
}

impl Inner {
	fn located(&self, ino: u64) -> Option<Located> {
		self.inodes.lock().ok()?.get(ino)
	}

	fn number(&self, at: &Located) -> u64 {
		self.inodes
			.lock()
			.map(|inodes| inodes.number(at))
			.unwrap_or(ROOT)
	}

	fn remember(&self, at: Located) -> u64 {
		self.inodes
			.lock()
			.map(|mut inodes| inodes.remember(at))
			.unwrap_or(ROOT)
	}

	fn next_fh(&self) -> u64 {
		self.next_fh.fetch_add(1, Ordering::Relaxed)
	}

	/// The arena's entry for `at`. `None` for a directory the arena has no
	/// entry for: the top of the mount, or a source root.
	async fn describe(&self, at: &Located) -> Result<Option<EntryMetadata>, MountError> {
		let Some(source) = at.source else {
			return Ok(None);
		};
		let arena = self
			.tree
			.arena(source, &at.abs)
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

	fn attr(&self, ino: u64, meta: Option<&EntryMetadata>) -> FileAttr {
		let (kind, size, perm, nlink) = match meta {
			Some(meta) if meta.kind != EntryKind::Directory => {
				(FileType::RegularFile, meta.size, 0o444, 1)
			}
			_ => (FileType::Directory, 0, 0o555, 2),
		};
		let mtime = meta
			.and_then(|meta| meta.modified)
			.unwrap_or(self.mounted_at);
		FileAttr {
			ino: INodeNo(ino),
			size,
			blocks: size.div_ceil(512),
			atime: meta.and_then(|meta| meta.accessed).unwrap_or(mtime),
			mtime,
			ctime: mtime,
			crtime: meta.and_then(|meta| meta.created).unwrap_or(mtime),
			kind,
			perm,
			nlink,
			uid: self.uid,
			gid: self.gid,
			rdev: 0,
			blksize: BLOCK_SIZE,
			flags: 0,
		}
	}

	async fn lookup(
		&self,
		parent: Located,
		name: &OsStr,
	) -> Result<(Duration, FileAttr), MountError> {
		if !plain(name) {
			return Err(MountError::NotFound);
		}
		let child = match parent.source {
			None => {
				let Layout::Shares(roots) = self.tree.layout().await else {
					return Err(MountError::NotFound);
				};
				roots
					.iter()
					.find(|root| OsStr::new(&root.name) == name)
					.map(Located::source_root)
					.ok_or(MountError::NotFound)?
			}
			Some(source) => Located {
				source: Some(source),
				abs: parent.abs.join(name),
				is_root: false,
				remote: parent.remote,
			},
		};
		let meta = self.describe(&child).await?;
		let ttl = child.ttl();
		let ino = self.remember(child);
		Ok((ttl, self.attr(ino, meta.as_ref())))
	}

	async fn list(&self, ino: u64, at: Located) -> Result<Vec<Listed>, MountError> {
		let mut listed = vec![
			Listed {
				ino,
				kind: FileType::Directory,
				name: OsString::from("."),
			},
			Listed {
				ino: ROOT,
				kind: FileType::Directory,
				name: OsString::from(".."),
			},
		];
		let Some(source) = at.source else {
			if let Layout::Shares(roots) = self.tree.layout().await {
				for root in roots {
					listed.push(Listed {
						ino: self.number(&Located::source_root(&root)),
						kind: FileType::Directory,
						name: OsString::from(root.name),
					});
				}
			}
			return Ok(listed);
		};
		if let Some(meta) = self.describe(&at).await? {
			if meta.kind != EntryKind::Directory {
				return Err(MountError::NotFound);
			}
		}
		self.tree.may_list(source, &at.abs).await?;
		let arena = self
			.tree
			.arena(source, &at.abs)
			.await
			.ok_or(MountError::NotFound)?;
		let arena = arena.read().await;
		for child in arena.list_directory(&at.abs).unwrap_or_default() {
			let (Some(meta), Some(name)) = (arena.get_entry_ref(&child), child.file_name()) else {
				continue;
			};
			let kind = match meta.kind {
				EntryKind::File => FileType::RegularFile,
				EntryKind::Directory => FileType::Directory,
				EntryKind::Symlink => continue,
			};
			let name = name.to_os_string();
			let located = Located {
				source: Some(source),
				abs: child.clone(),
				is_root: false,
				remote: at.remote,
			};
			listed.push(Listed {
				ino: self.number(&located),
				kind,
				name,
			});
		}
		Ok(listed)
	}

	async fn open(&self, at: Located) -> Result<OpenFile, MountError> {
		let Some(source) = at.source else {
			return Err(MountError::Denied("a directory".to_string()));
		};
		let provider = self.tree.open(source, &at.abs).await?;
		Ok(OpenFile {
			provider,
			target: ByteTarget {
				source_id: source,
				path: at.abs,
			},
		})
	}
}

/// Bytes `[offset, offset + size)` of an open file, short only at its end.
async fn read(file: &OpenFile, offset: u64, size: u32) -> Result<Vec<u8>, MountError> {
	let end = offset.saturating_add(size as u64);
	let mut out = Vec::with_capacity(size as usize);
	let mut cursor = offset;
	while cursor < end {
		let want = (end - cursor).min(file.provider.max_read());
		let chunk = file
			.provider
			.read_range(&file.target, cursor..cursor + want)
			.await?;
		if chunk.is_empty() {
			break;
		}
		// A cached provider answers at most to the end of one block, so only an
		// empty chunk means the end of the file. The kernel fills a short reply
		// with zeroes, so stopping early would corrupt the read.
		cursor += chunk.len() as u64;
		out.extend_from_slice(&chunk);
	}
	Ok(out)
}

impl SpacedriveFs {
	fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
		self.runtime.spawn(task);
	}
}

impl Filesystem for SpacedriveFs {
	fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
		let Some(parent) = self.inner.located(parent.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		let name = name.to_os_string();
		self.spawn(async move {
			match inner.lookup(parent, &name).await {
				Ok((ttl, attr)) => reply.entry(&ttl, &attr, Generation(0)),
				Err(err) => reply.error(errno(&err)),
			}
		});
	}

	fn forget(&self, _req: &Request, ino: INodeNo, nlookup: u64) {
		if let Ok(mut inodes) = self.inner.inodes.lock() {
			inodes.forget(ino.0, nlookup);
		}
	}

	fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
		let Some(at) = self.inner.located(ino.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		self.spawn(async move {
			match inner.describe(&at).await {
				Ok(meta) => reply.attr(&at.ttl(), &inner.attr(ino.0, meta.as_ref())),
				Err(err) => reply.error(errno(&err)),
			}
		});
	}

	fn open(
		&self,
		_req: &Request,
		ino: INodeNo,
		flags: OpenFlags,
		_kill_suid_gid: bool,
		reply: ReplyOpen,
	) {
		if flags.acc_mode() != OpenAccMode::O_RDONLY {
			return reply.error(Errno::EROFS);
		}
		let Some(at) = self.inner.located(ino.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		self.spawn(async move {
			match inner.describe(&at).await {
				Ok(Some(meta)) if meta.kind == EntryKind::File => {}
				Ok(_) => return reply.error(Errno::EISDIR),
				Err(err) => return reply.error(errno(&err)),
			}
			match inner.open(at).await {
				Ok(file) => {
					let fh = inner.next_fh();
					if let Ok(mut files) = inner.files.lock() {
						files.insert(fh, Arc::new(file));
					}
					reply.opened(FileHandle(fh), FopenFlags::empty());
				}
				Err(err) => {
					debug!(%err, "fuse open refused");
					reply.error(errno(&err));
				}
			}
		});
	}

	fn read(
		&self,
		_req: &Request,
		_ino: INodeNo,
		fh: FileHandle,
		offset: u64,
		size: u32,
		_flags: OpenFlags,
		_lock_owner: Option<LockOwner>,
		reply: ReplyData,
	) {
		let file = self
			.inner
			.files
			.lock()
			.ok()
			.and_then(|files| files.get(&fh.0).cloned());
		let Some(file) = file else {
			return reply.error(Errno::EBADF);
		};
		self.spawn(async move {
			match read(&file, offset, size).await {
				Ok(bytes) => reply.data(&bytes),
				Err(err) => {
					debug!(%err, "fuse read failed");
					reply.error(errno(&err));
				}
			}
		});
	}

	fn flush(
		&self,
		_req: &Request,
		_ino: INodeNo,
		_fh: FileHandle,
		_lock_owner: LockOwner,
		reply: ReplyEmpty,
	) {
		reply.ok();
	}

	fn release(
		&self,
		_req: &Request,
		_ino: INodeNo,
		fh: FileHandle,
		_flags: OpenFlags,
		_lock_owner: Option<LockOwner>,
		_flush: bool,
		reply: ReplyEmpty,
	) {
		if let Ok(mut files) = self.inner.files.lock() {
			files.remove(&fh.0);
		}
		reply.ok();
	}

	fn opendir(&self, _req: &Request, ino: INodeNo, _flags: OpenFlags, reply: ReplyOpen) {
		let Some(at) = self.inner.located(ino.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		self.spawn(async move {
			match inner.list(ino.0, at).await {
				Ok(listed) => {
					let fh = inner.next_fh();
					if let Ok(mut dirs) = inner.dirs.lock() {
						dirs.insert(fh, Arc::new(listed));
					}
					reply.opened(FileHandle(fh), FopenFlags::empty());
				}
				Err(err) => {
					debug!(%err, "fuse listing refused");
					reply.error(errno(&err));
				}
			}
		});
	}

	fn readdir(
		&self,
		_req: &Request,
		_ino: INodeNo,
		fh: FileHandle,
		offset: u64,
		mut reply: ReplyDirectory,
	) {
		let listed = self
			.inner
			.dirs
			.lock()
			.ok()
			.and_then(|dirs| dirs.get(&fh.0).cloned());
		let Some(listed) = listed else {
			return reply.error(Errno::EBADF);
		};
		for (index, entry) in listed.iter().enumerate().skip(offset as usize) {
			if reply.add(
				INodeNo(entry.ino),
				index as u64 + 1,
				entry.kind,
				&entry.name,
			) {
				break;
			}
		}
		reply.ok();
	}

	fn releasedir(
		&self,
		_req: &Request,
		_ino: INodeNo,
		fh: FileHandle,
		_flags: OpenFlags,
		reply: ReplyEmpty,
	) {
		if let Ok(mut dirs) = self.inner.dirs.lock() {
			dirs.remove(&fh.0);
		}
		reply.ok();
	}

	fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
		reply.statfs(0, 0, 0, 0, 0, BLOCK_SIZE, 255, BLOCK_SIZE);
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn at(source: Uuid, path: &str) -> Located {
		Located {
			source: Some(source),
			abs: PathBuf::from(path),
			is_root: false,
			remote: false,
		}
	}

	#[test]
	fn a_path_keeps_its_number_until_forgotten() {
		let source = Uuid::now_v7();
		let mut inodes = Inodes::new(Located {
			source: None,
			abs: PathBuf::new(),
			is_root: true,
			remote: false,
		});
		let listed = inodes.number(&at(source, "/data/a.txt"));
		let first = inodes.remember(at(source, "/data/a.txt"));
		assert_eq!(listed, first, "a listing reports the number a lookup gives");
		assert_eq!(inodes.remember(at(source, "/data/a.txt")), first);
		assert_ne!(inodes.remember(at(source, "/data/b.txt")), first);

		inodes.forget(first, 1);
		assert!(inodes.get(first).is_some(), "one lookup is still held");
		inodes.forget(first, 1);
		assert!(inodes.get(first).is_none());

		inodes.forget(ROOT, 10);
		assert!(inodes.get(ROOT).is_some(), "the root is never forgotten");
	}

	#[test]
	fn only_plain_names_are_looked_up() {
		assert!(plain(OsStr::new("notes.md")));
		assert!(!plain(OsStr::new("..")));
		assert!(!plain(OsStr::new(".")));
		assert!(!plain(OsStr::new("a/b")));
		assert!(!plain(OsStr::new("")));
	}
}
