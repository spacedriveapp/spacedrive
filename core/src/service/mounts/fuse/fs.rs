//! The kernel side of a FUSE mount: inode numbers, attributes and replies.
//!
//! fuser calls every method on its session thread and expects no async. Each
//! call becomes a task on the runtime that owns its reply, so a slow remote
//! read never holds up a listing on the same mount. Everything the mount
//! shows comes from a [`MountFs`], so the daemon's sources and a thin
//! client's remote space mount through the same code.
//!
//! Inode numbers are a hash of the path, so a path keeps its number for as
//! long as the kernel remembers it and a listing can report one without
//! allocating anything. Only looked-up paths are kept, and each is dropped
//! when the kernel forgets it.

use crate::service::mounts::vfs::{plain, MountError, MountFs, NodeAttr, NodeKind, OpenedFile};
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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tokio::runtime::Handle;
use tracing::debug;

const ROOT: u64 = 1;
/// Reported as the preferred I/O size, so tools read in large ranges.
const BLOCK_SIZE: u32 = 128 * 1024;

/// A running mount. Dropping it unmounts.
pub struct Mounted {
	session: Option<BackgroundSession>,
	invalidations: Option<tokio::task::JoinHandle<()>>,
}

impl Mounted {
	/// Unmount, waiting for the kernel to let go.
	pub fn umount_and_join(mut self) -> std::io::Result<()> {
		match self.session.take() {
			Some(session) => session.umount_and_join(),
			None => Ok(()),
		}
	}
}

impl Drop for Mounted {
	fn drop(&mut self) {
		if let Some(task) = &self.invalidations {
			task.abort();
		}
	}
}

/// Mount `fs` at `mountpoint`, read-only. The mount lasts until the returned
/// session is unmounted or dropped.
pub async fn mount(fs: Arc<dyn MountFs>, mountpoint: &Path) -> std::io::Result<Mounted> {
	let changes = fs.changes();
	let inner = Arc::new(Inner {
		fs,
		inodes: Mutex::new(Inodes::new()),
		files: Mutex::new(HashMap::new()),
		dirs: Mutex::new(HashMap::new()),
		next_fh: AtomicU64::new(1),
		// Owned by the mounting user, the only one the kernel lets in without
		// allow_other.
		uid: unsafe { libc::getuid() },
		gid: unsafe { libc::getgid() },
		mounted_at: SystemTime::now(),
	});
	let driver = SpacedriveFs {
		inner: inner.clone(),
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
	let session =
		tokio::task::spawn_blocking(move || fuser::spawn_mount(driver, &mountpoint, &config))
			.await
			.map_err(std::io::Error::other)??;
	let invalidations = changes.map(|changes| {
		let notifier = session.notifier();
		tokio::spawn(invalidate(inner, notifier, changes))
	});
	Ok(Mounted {
		session: Some(session),
		invalidations,
	})
}

/// Tell the kernel to drop what it cached for each changed path: the file's
/// attributes and pages, and the name in its parent so a lookup asks again.
async fn invalidate(
	inner: Arc<Inner>,
	notifier: fuser::Notifier,
	mut changes: tokio::sync::broadcast::Receiver<PathBuf>,
) {
	use tokio::sync::broadcast::error::RecvError;
	loop {
		let path = match changes.recv().await {
			Ok(path) => path,
			// Missed changes are covered by each answer's validity period.
			Err(RecvError::Lagged(_)) => continue,
			Err(RecvError::Closed) => break,
		};
		let (ino, parent) = {
			let Ok(inodes) = inner.inodes.lock() else {
				continue;
			};
			let parent = path
				.parent()
				.and_then(|parent| inodes.known(parent))
				.zip(path.file_name().map(OsStr::to_os_string));
			(inodes.known(&path), parent)
		};
		let notifier = notifier.clone();
		let _ = tokio::task::spawn_blocking(move || {
			if let Some(ino) = ino {
				let _ = notifier.inval_inode(INodeNo(ino), 0, 0);
			}
			if let Some((parent, name)) = parent {
				let _ = notifier.inval_entry(INodeNo(parent), &name);
				let _ = notifier.inval_inode(INodeNo(parent), 0, 0);
			}
		})
		.await;
	}
}

struct Node {
	path: PathBuf,
	lookups: u64,
}

struct Inodes {
	nodes: HashMap<u64, Node>,
}

impl Inodes {
	fn new() -> Self {
		let mut nodes = HashMap::new();
		nodes.insert(
			ROOT,
			Node {
				path: PathBuf::new(),
				lookups: 1,
			},
		);
		Self { nodes }
	}

	fn get(&self, ino: u64) -> Option<PathBuf> {
		self.nodes.get(&ino).map(|node| node.path.clone())
	}

	/// The number `path` has, or would get if it were looked up now.
	fn number(&self, path: &Path) -> u64 {
		if path.as_os_str().is_empty() {
			return ROOT;
		}
		let mut hasher = DefaultHasher::new();
		path.hash(&mut hasher);
		let mut ino = hasher.finish().max(ROOT + 1);
		while let Some(node) = self.nodes.get(&ino) {
			if node.path == path {
				break;
			}
			ino = ino.wrapping_add(1).max(ROOT + 1);
		}
		ino
	}

	/// The number of `path` if the kernel holds it.
	fn known(&self, path: &Path) -> Option<u64> {
		let ino = self.number(path);
		self.nodes.contains_key(&ino).then_some(ino)
	}

	fn remember(&mut self, path: PathBuf) -> u64 {
		let ino = self.number(&path);
		if ino == ROOT {
			return ROOT;
		}
		self.nodes
			.entry(ino)
			.or_insert(Node { path, lookups: 0 })
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

struct Listed {
	ino: u64,
	kind: FileType,
	name: OsString,
}

struct Inner {
	fs: Arc<dyn MountFs>,
	inodes: Mutex<Inodes>,
	files: Mutex<HashMap<u64, Arc<OpenedFile>>>,
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
		MountError::NotADirectory => Errno::ENOTDIR,
		MountError::IsADirectory => Errno::EISDIR,
	}
}

fn file_type(kind: NodeKind) -> FileType {
	match kind {
		NodeKind::File => FileType::RegularFile,
		NodeKind::Directory => FileType::Directory,
	}
}

impl Inner {
	fn path(&self, ino: u64) -> Option<PathBuf> {
		self.inodes.lock().ok()?.get(ino)
	}

	fn number(&self, path: &Path) -> u64 {
		self.inodes
			.lock()
			.map(|inodes| inodes.number(path))
			.unwrap_or(ROOT)
	}

	fn remember(&self, path: PathBuf) -> u64 {
		self.inodes
			.lock()
			.map(|mut inodes| inodes.remember(path))
			.unwrap_or(ROOT)
	}

	fn next_fh(&self) -> u64 {
		self.next_fh.fetch_add(1, Ordering::Relaxed)
	}

	fn attr(&self, ino: u64, node: &NodeAttr) -> FileAttr {
		let (perm, nlink) = match node.kind {
			NodeKind::File => (0o444, 1),
			NodeKind::Directory => (0o555, 2),
		};
		let mtime = node.modified.unwrap_or(self.mounted_at);
		FileAttr {
			ino: INodeNo(ino),
			size: node.size,
			blocks: node.size.div_ceil(512),
			atime: node.accessed.unwrap_or(mtime),
			mtime,
			ctime: mtime,
			crtime: node.created.unwrap_or(mtime),
			kind: file_type(node.kind),
			perm,
			nlink,
			uid: self.uid,
			gid: self.gid,
			rdev: 0,
			blksize: BLOCK_SIZE,
			flags: 0,
		}
	}

	async fn lookup(&self, parent: PathBuf, name: &OsStr) -> Result<(NodeAttr, u64), MountError> {
		if !plain(name) {
			return Err(MountError::NotFound);
		}
		let child = parent.join(name);
		let node = self.fs.stat(&child).await?;
		let ino = self.remember(child);
		Ok((node, ino))
	}

	async fn list(&self, ino: u64, path: PathBuf) -> Result<Vec<Listed>, MountError> {
		let parent = path
			.parent()
			.map(|parent| self.number(parent))
			.unwrap_or(ROOT);
		let mut listed = vec![
			Listed {
				ino,
				kind: FileType::Directory,
				name: OsString::from("."),
			},
			Listed {
				ino: parent,
				kind: FileType::Directory,
				name: OsString::from(".."),
			},
		];
		for entry in self.fs.list(&path).await? {
			if !plain(&entry.name) {
				continue;
			}
			listed.push(Listed {
				ino: self.number(&path.join(&entry.name)),
				kind: file_type(entry.kind),
				name: entry.name,
			});
		}
		Ok(listed)
	}
}

impl SpacedriveFs {
	fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
		self.runtime.spawn(task);
	}
}

impl Filesystem for SpacedriveFs {
	fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
		let Some(parent) = self.inner.path(parent.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		let name = name.to_os_string();
		self.spawn(async move {
			match inner.lookup(parent, &name).await {
				Ok((node, ino)) => {
					reply.entry(&node.valid_for, &inner.attr(ino, &node), Generation(0))
				}
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
		let Some(path) = self.inner.path(ino.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		self.spawn(async move {
			match inner.fs.stat(&path).await {
				Ok(node) => reply.attr(&node.valid_for, &inner.attr(ino.0, &node)),
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
		let Some(path) = self.inner.path(ino.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		self.spawn(async move {
			match inner.fs.open(&path).await {
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
			match file.read(offset, size).await {
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
		let Some(path) = self.inner.path(ino.0) else {
			return reply.error(Errno::ENOENT);
		};
		let inner = self.inner.clone();
		self.spawn(async move {
			match inner.list(ino.0, path).await {
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

	#[test]
	fn a_path_keeps_its_number_until_forgotten() {
		let mut inodes = Inodes::new();
		let listed = inodes.number(Path::new("data/a.txt"));
		let first = inodes.remember(PathBuf::from("data/a.txt"));
		assert_eq!(listed, first, "a listing reports the number a lookup gives");
		assert_eq!(inodes.known(Path::new("data/a.txt")), Some(first));
		assert_eq!(inodes.remember(PathBuf::from("data/a.txt")), first);
		assert_ne!(inodes.remember(PathBuf::from("data/b.txt")), first);

		inodes.forget(first, 1);
		assert!(inodes.get(first).is_some(), "one lookup is still held");
		inodes.forget(first, 1);
		assert!(inodes.get(first).is_none());
		assert_eq!(inodes.known(Path::new("data/a.txt")), None);

		inodes.forget(ROOT, 10);
		assert!(inodes.get(ROOT).is_some(), "the root is never forgotten");
		assert_eq!(inodes.number(Path::new("")), ROOT);
	}
}
