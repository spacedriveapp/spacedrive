//! # NFS adapter and server
//!
//! The NFSv3 side of a mount: file ids, attributes and replies.
//!
//! [`NfsAdapter`] answers nfsserve's [`NFSFileSystem`] calls from a [`MountFs`], so
//! the daemon's sources and a thin client's remote space are served by the
//! same code. NFS names files by a 64-bit id that the client keeps for as
//! long as it likes, so an id is a hash of the file's path and every id the
//! server has handed out stays known while it runs. The root is always 1.
//!
//! NFSv3 has no open or close, so the adapter keeps the last few opened files
//! in a small cache keyed by id: sequential reads of one file reuse its
//! provider instead of opening it again for every request. Cached files and
//! listings are dropped when the backend's change feed names their path,
//! and they expire after a short while either way, so a grant bound to the
//! mount is asked again before long.
//!
//! [`serve`] binds the adapter on 127.0.0.1 on a port the OS picks, behind a
//! small front that fixes nfsserve's READDIR paging (see `front`), and
//! [`mount`] and [`unmount`] ask the OS's own NFS client to attach it.
//!
//! ## Example
//! ```rust,no_run
//! # async fn demo(fs: std::sync::Arc<dyn sd_core::service::mounts::vfs::MountFs>) -> std::io::Result<()> {
//! use sd_core::service::mounts::nfs::server;
//! use std::path::Path;
//!
//! let nfs = server::serve(fs).await?;
//! server::mount(&nfs, Path::new("/Users/me/sd")).await?;
//! // ...
//! server::unmount(Path::new("/Users/me/sd")).await?;
//! nfs.shutdown();
//! # Ok(())
//! # }
//! ```

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::hash::{Hash, Hasher};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use nfsserve::nfs::{
	cookieverf3, fattr3, fileid3, filename3, fsinfo3, ftype3, nfs_fh3, nfspath3, nfsstat3,
	nfstime3, post_op_attr, sattr3, specdata3,
};
use nfsserve::tcp::{NFSTcp, NFSTcpListener};
use nfsserve::vfs::{DirEntry, NFSFileSystem, ReadDirResult, VFSCapabilities};
use rand::RngCore;
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{debug, info};

use crate::service::mounts::vfs::{plain, MountError, MountFs, NodeAttr, NodeKind, OpenedFile};

pub(super) const ROOT: fileid3 = 1;
/// Open files kept between reads. Sequential readers touch one or two files
/// at a time; the bound keeps providers from piling up.
const OPEN_FILES: usize = 16;
/// Directories whose listing is kept between readdir pages.
const LISTINGS: usize = 32;
/// How long an opened file or a listing is reused. Long enough for one
/// sequential read or one paged listing, short enough that a revoked grant
/// or a changed directory is noticed without a change feed.
const REUSE_FOR: Duration = Duration::from_secs(10);
/// Every file and directory reports this filesystem id.
const FSID: u64 = 0x5344_4d4e_5446_5331;
/// Largest read and preferred read size the client is offered. Large reads
/// mean fewer round trips for a peer's file.
const READ_MAX: u32 = 1024 * 1024;
/// Bytes of keyed tag after the file id in every file handle.
const HANDLE_TAG: usize = 16;

/// Random bytes from the OS, for the per-server secrets.
fn secret<const N: usize>() -> [u8; N] {
	let mut bytes = [0u8; N];
	rand::rngs::OsRng.fill_bytes(&mut bytes);
	bytes
}

/// The one export path a server answers MOUNT for: 128 random bits that
/// exist only in this process and in the mount command. The path also shows
/// in the OS's mount table, so the front honors it once and refuses every
/// later MOUNT, whoever sends it.
pub(super) struct Export {
	path: String,
	claimed: AtomicBool,
}

impl Export {
	fn new() -> Self {
		Self {
			path: format!("/{}", hex::encode(secret::<16>())),
			claimed: AtomicBool::new(false),
		}
	}

	/// Whether a MOUNT for `path` may have the root handle. Only the first
	/// MOUNT for the right path gets it.
	pub(super) fn claim(&self, path: &[u8]) -> bool {
		let wanted = self.path.trim_start_matches('/').as_bytes();
		let asked = trim_slashes(path);
		let matches = asked.len() == wanted.len()
			&& asked
				.iter()
				.zip(wanted)
				.fold(0u8, |diff, (a, b)| diff | (a ^ b))
				== 0;
		matches && !self.claimed.swap(true, Ordering::SeqCst)
	}
}

fn trim_slashes(mut path: &[u8]) -> &[u8] {
	while let [b'/', rest @ ..] = path {
		path = rest;
	}
	while let [rest @ .., b'/'] = path {
		path = rest;
	}
	path
}

/// Ids the server has handed out, both ways.
struct Ids {
	paths: HashMap<fileid3, PathBuf>,
	ids: HashMap<PathBuf, fileid3>,
}

impl Ids {
	fn new() -> Self {
		let mut ids = Self {
			paths: HashMap::new(),
			ids: HashMap::new(),
		};
		ids.paths.insert(ROOT, PathBuf::new());
		ids.ids.insert(PathBuf::new(), ROOT);
		ids
	}

	fn path(&self, id: fileid3) -> Option<PathBuf> {
		self.paths.get(&id).cloned()
	}

	/// The id of `path`, the same one every time while the server runs.
	fn id(&mut self, path: &Path) -> fileid3 {
		if let Some(id) = self.ids.get(path) {
			return *id;
		}
		let mut hasher = DefaultHasher::new();
		path.hash(&mut hasher);
		// 0 is reserved by NFS and 1 is the root.
		let mut id = hasher.finish().max(ROOT + 1);
		while self.paths.contains_key(&id) {
			id = id.wrapping_add(1).max(ROOT + 1);
		}
		self.paths.insert(id, path.to_path_buf());
		self.ids.insert(path.to_path_buf(), id);
		id
	}
}

/// One child of a listed directory, in listing order.
pub(super) struct Listed {
	pub(super) id: fileid3,
	pub(super) name: OsString,
}

struct Listing {
	at: Instant,
	entries: Arc<Vec<Listed>>,
}

struct Opened {
	at: Instant,
	file: Arc<OpenedFile>,
	/// The version the provider was opened at. A provider keeps reading
	/// the file it opened, so a replaced file needs a new one.
	size: u64,
	modified: Option<SystemTime>,
}

struct Inner {
	fs: Arc<dyn MountFs>,
	ids: Mutex<Ids>,
	listings: Mutex<HashMap<fileid3, Listing>>,
	/// Most recently used last.
	files: Mutex<Vec<(fileid3, Opened)>>,
	closed: AtomicBool,
	uid: u32,
	gid: u32,
	started: SystemTime,
	/// Keys the tag on every file handle, so a handle is only good if this
	/// server handed it out.
	handle_key: [u8; 32],
}

/// A [`MountFs`] answering NFSv3 calls, read-only.
#[derive(Clone)]
pub struct NfsAdapter {
	inner: Arc<Inner>,
}

fn nfs_error(err: &MountError) -> nfsstat3 {
	match err {
		MountError::NotFound => nfsstat3::NFS3ERR_NOENT,
		MountError::Denied(_) => nfsstat3::NFS3ERR_ACCES,
		MountError::Unavailable(_) => nfsstat3::NFS3ERR_IO,
		MountError::NotADirectory => nfsstat3::NFS3ERR_NOTDIR,
		MountError::IsADirectory => nfsstat3::NFS3ERR_ISDIR,
	}
}

fn nfs_time(time: SystemTime) -> nfstime3 {
	let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
	nfstime3 {
		seconds: since.as_secs().min(u32::MAX as u64) as u32,
		nseconds: since.subsec_nanos(),
	}
}

#[cfg(unix)]
fn os_name(name: &filename3) -> OsString {
	use std::os::unix::ffi::OsStrExt;
	OsStr::from_bytes(name).to_os_string()
}

#[cfg(not(unix))]
fn os_name(name: &filename3) -> OsString {
	OsString::from(String::from_utf8_lossy(name).into_owned())
}

#[cfg(unix)]
pub(super) fn nfs_name(name: &OsStr) -> filename3 {
	use std::os::unix::ffi::OsStrExt;
	name.as_bytes().into()
}

#[cfg(not(unix))]
pub(super) fn nfs_name(name: &OsStr) -> filename3 {
	name.to_string_lossy().as_bytes().into()
}

#[cfg(unix)]
fn owner() -> (u32, u32) {
	// Owned by the serving user; the client checks the mode bits, which let
	// everyone read.
	unsafe { (libc::getuid(), libc::getgid()) }
}

#[cfg(not(unix))]
fn owner() -> (u32, u32) {
	(0, 0)
}

impl NfsAdapter {
	pub fn new(fs: Arc<dyn MountFs>) -> Self {
		let (uid, gid) = owner();
		Self {
			inner: Arc::new(Inner {
				fs,
				ids: Mutex::new(Ids::new()),
				listings: Mutex::new(HashMap::new()),
				files: Mutex::new(Vec::new()),
				closed: AtomicBool::new(false),
				uid,
				gid,
				started: SystemTime::now(),
				handle_key: secret(),
			}),
		}
	}

	/// Refuse every call from now on, so a client that still holds the
	/// mount gets errors instead of the files.
	pub fn close(&self) {
		self.inner.closed.store(true, Ordering::SeqCst);
		if let Ok(mut files) = self.inner.files.lock() {
			files.clear();
		}
		if let Ok(mut listings) = self.inner.listings.lock() {
			listings.clear();
		}
	}

	/// Drop what is cached for `path`, its parent's listing, and anything
	/// under it. The empty path drops everything.
	pub fn invalidate(&self, path: &Path) {
		let ids: Vec<fileid3> = match self.inner.ids.lock() {
			Ok(ids) => ids
				.ids
				.iter()
				.filter(|(known, _)| {
					known.starts_with(path) || path.parent().is_some_and(|parent| *known == parent)
				})
				.map(|(_, id)| *id)
				.collect(),
			Err(_) => return,
		};
		if let Ok(mut listings) = self.inner.listings.lock() {
			listings.retain(|id, _| !ids.contains(id));
		}
		if let Ok(mut files) = self.inner.files.lock() {
			files.retain(|(id, _)| !ids.contains(id));
		}
	}

	fn handle_tag(&self, id: fileid3) -> [u8; HANDLE_TAG] {
		let hash = blake3::keyed_hash(&self.inner.handle_key, &id.to_le_bytes());
		let mut tag = [0u8; HANDLE_TAG];
		tag.copy_from_slice(&hash.as_bytes()[..HANDLE_TAG]);
		tag
	}

	fn open_for_calls(&self) -> Result<(), nfsstat3> {
		if self.inner.closed.load(Ordering::SeqCst) {
			return Err(nfsstat3::NFS3ERR_IO);
		}
		Ok(())
	}

	fn path(&self, id: fileid3) -> Result<PathBuf, nfsstat3> {
		self.open_for_calls()?;
		self.inner
			.ids
			.lock()
			.ok()
			.and_then(|ids| ids.path(id))
			.ok_or(nfsstat3::NFS3ERR_STALE)
	}

	fn id(&self, path: &Path) -> fileid3 {
		self.inner
			.ids
			.lock()
			.map(|mut ids| ids.id(path))
			.unwrap_or(ROOT)
	}

	fn attr(&self, id: fileid3, node: &NodeAttr) -> fattr3 {
		let (ftype, mode, nlink) = match node.kind {
			NodeKind::File => (ftype3::NF3REG, 0o444, 1),
			NodeKind::Directory => (ftype3::NF3DIR, 0o555, 2),
		};
		// A directory without a time, such as a source root, reports now: the
		// client refreshes a cached listing only when the directory's mtime
		// moves, and these listings change without one.
		let mtime = match (node.modified, node.kind) {
			(Some(modified), _) => modified,
			(None, NodeKind::Directory) => SystemTime::now(),
			(None, NodeKind::File) => self.inner.started,
		};
		fattr3 {
			ftype,
			mode,
			nlink,
			uid: self.inner.uid,
			gid: self.inner.gid,
			size: node.size,
			used: node.size,
			rdev: specdata3::default(),
			fsid: FSID,
			fileid: id,
			atime: nfs_time(node.accessed.unwrap_or(mtime)),
			mtime: nfs_time(mtime),
			ctime: nfs_time(mtime),
		}
	}

	/// The children of `dir` in a stable order. A listing that starts at the
	/// top is read fresh; the pages after it reuse it, so the cookies a
	/// client holds keep meaning the same entries.
	async fn listing(&self, dir: fileid3, fresh: bool) -> Result<Arc<Vec<Listed>>, nfsstat3> {
		let path = self.path(dir)?;
		if !fresh {
			let cached = self.inner.listings.lock().ok().and_then(|listings| {
				listings
					.get(&dir)
					.filter(|listing| listing.at.elapsed() < REUSE_FOR)
					.map(|listing| listing.entries.clone())
			});
			if let Some(entries) = cached {
				return Ok(entries);
			}
		}
		let mut children = self
			.inner
			.fs
			.list(&path)
			.await
			.map_err(|err| nfs_error(&err))?;
		children.retain(|child| plain(&child.name));
		children.sort_by(|a, b| a.name.cmp(&b.name));
		let entries: Arc<Vec<Listed>> = Arc::new(
			children
				.into_iter()
				.map(|child| Listed {
					id: self.id(&path.join(&child.name)),
					name: child.name,
				})
				.collect(),
		);
		if let Ok(mut listings) = self.inner.listings.lock() {
			if listings.len() >= LISTINGS && !listings.contains_key(&dir) {
				if let Some(oldest) = listings
					.iter()
					.min_by_key(|(_, listing)| listing.at)
					.map(|(id, _)| *id)
				{
					listings.remove(&oldest);
				}
			}
			listings.insert(
				dir,
				Listing {
					at: Instant::now(),
					entries: entries.clone(),
				},
			);
		}
		Ok(entries)
	}

	/// The listing of `dir` and the index of the entry after `cookie`, where
	/// a cookie is the id of the last entry a client was given and 0 is the
	/// start.
	pub(super) async fn page(
		&self,
		dir: fileid3,
		cookie: u64,
	) -> Result<(Arc<Vec<Listed>>, usize), nfsstat3> {
		let entries = self.listing(dir, cookie == 0).await?;
		if cookie == 0 {
			return Ok((entries, 0));
		}
		let start = entries
			.iter()
			.position(|entry| entry.id == cookie)
			.map(|index| index + 1)
			.ok_or(nfsstat3::NFS3ERR_BAD_COOKIE)?;
		Ok((entries, start))
	}

	/// The opened file behind `id`, from the cache while it is fresh and the
	/// file has the size and time it was opened at.
	async fn opened(&self, id: fileid3) -> Result<(Arc<OpenedFile>, u64), nfsstat3> {
		let path = self.path(id)?;
		let node = self
			.inner
			.fs
			.stat(&path)
			.await
			.map_err(|err| nfs_error(&err))?;
		if node.is_dir() {
			return Err(nfsstat3::NFS3ERR_ISDIR);
		}
		if let Ok(mut files) = self.inner.files.lock() {
			if let Some(index) = files.iter().position(|(known, _)| *known == id) {
				let (known, opened) = files.remove(index);
				if opened.at.elapsed() < REUSE_FOR
					&& opened.size == node.size
					&& opened.modified == node.modified
				{
					let file = opened.file.clone();
					files.push((known, opened));
					return Ok((file, node.size));
				}
			}
		}
		let file = Arc::new(self.inner.fs.open(&path).await.map_err(|err| {
			debug!(%err, path = %path.display(), "nfs open refused");
			nfs_error(&err)
		})?);
		if let Ok(mut files) = self.inner.files.lock() {
			files.retain(|(known, _)| *known != id);
			if files.len() >= OPEN_FILES {
				files.remove(0);
			}
			files.push((
				id,
				Opened {
					at: Instant::now(),
					file: file.clone(),
					size: node.size,
					modified: node.modified,
				},
			));
		}
		Ok((file, node.size))
	}
}

#[async_trait]
impl NFSFileSystem for NfsAdapter {
	fn capabilities(&self) -> VFSCapabilities {
		VFSCapabilities::ReadOnly
	}

	fn root_dir(&self) -> fileid3 {
		ROOT
	}

	async fn lookup(&self, dirid: fileid3, filename: &filename3) -> Result<fileid3, nfsstat3> {
		let dir = self.path(dirid)?;
		let name = os_name(filename);
		if name == "." {
			return Ok(dirid);
		}
		if name == ".." {
			return Ok(dir.parent().map(|parent| self.id(parent)).unwrap_or(ROOT));
		}
		if !plain(&name) {
			return Err(nfsstat3::NFS3ERR_NOENT);
		}
		let child = dir.join(&name);
		self.inner
			.fs
			.stat(&child)
			.await
			.map_err(|err| nfs_error(&err))?;
		Ok(self.id(&child))
	}

	async fn getattr(&self, id: fileid3) -> Result<fattr3, nfsstat3> {
		let path = self.path(id)?;
		let node = match self.inner.fs.stat(&path).await {
			Ok(node) => node,
			// A client that cannot stat the mount root reports a stale handle
			// rather than the refusal, so the root keeps its attributes and
			// the listing itself carries the refusal.
			Err(MountError::Denied(_)) if id == ROOT => NodeAttr::directory(Duration::ZERO),
			Err(err) => return Err(nfs_error(&err)),
		};
		Ok(self.attr(id, &node))
	}

	async fn setattr(&self, _id: fileid3, _setattr: sattr3) -> Result<fattr3, nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn read(
		&self,
		id: fileid3,
		offset: u64,
		count: u32,
	) -> Result<(Vec<u8>, bool), nfsstat3> {
		let (file, size) = self.opened(id).await?;
		let bytes = file.read(offset, count).await.map_err(|err| {
			debug!(%err, "nfs read failed");
			nfs_error(&err)
		})?;
		let eof = (bytes.len() as u64) < count as u64 || offset + bytes.len() as u64 >= size;
		Ok((bytes, eof))
	}

	async fn write(&self, _id: fileid3, _offset: u64, _data: &[u8]) -> Result<fattr3, nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn create(
		&self,
		_dirid: fileid3,
		_filename: &filename3,
		_attr: sattr3,
	) -> Result<(fileid3, fattr3), nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn create_exclusive(
		&self,
		_dirid: fileid3,
		_filename: &filename3,
	) -> Result<fileid3, nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn mkdir(
		&self,
		_dirid: fileid3,
		_dirname: &filename3,
	) -> Result<(fileid3, fattr3), nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn remove(&self, _dirid: fileid3, _filename: &filename3) -> Result<(), nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn rename(
		&self,
		_from_dirid: fileid3,
		_from_filename: &filename3,
		_to_dirid: fileid3,
		_to_filename: &filename3,
	) -> Result<(), nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn readdir(
		&self,
		dirid: fileid3,
		start_after: fileid3,
		max_entries: usize,
	) -> Result<ReadDirResult, nfsstat3> {
		let (entries, start) = self.page(dirid, start_after).await?;
		let end = (start + max_entries.max(1)).min(entries.len());
		let dir = self.path(dirid)?;
		let page = &entries[start..end];
		let paths: Vec<PathBuf> = page.iter().map(|entry| dir.join(&entry.name)).collect();
		let stats =
			futures::future::join_all(paths.iter().map(|path| self.inner.fs.stat(path))).await;
		let mut result = ReadDirResult {
			entries: Vec::with_capacity(page.len()),
			end: end == entries.len(),
		};
		for (entry, stat) in page.iter().zip(stats) {
			// A child that vanished since the listing is left out; the
			// cookies of the entries around it still hold.
			let Ok(node) = stat else {
				continue;
			};
			result.entries.push(DirEntry {
				fileid: entry.id,
				name: nfs_name(&entry.name),
				attr: self.attr(entry.id, &node),
			});
		}
		Ok(result)
	}

	async fn symlink(
		&self,
		_dirid: fileid3,
		_linkname: &filename3,
		_symlink: &nfspath3,
		_attr: &sattr3,
	) -> Result<(fileid3, fattr3), nfsstat3> {
		Err(nfsstat3::NFS3ERR_ROFS)
	}

	async fn readlink(&self, id: fileid3) -> Result<nfspath3, nfsstat3> {
		self.path(id)?;
		// The mount serves no symlinks.
		Err(nfsstat3::NFS3ERR_INVAL)
	}

	async fn fsinfo(&self, root_fileid: fileid3) -> Result<fsinfo3, nfsstat3> {
		let obj_attributes = match self.getattr(root_fileid).await {
			Ok(attr) => post_op_attr::attributes(attr),
			Err(_) => post_op_attr::Void,
		};
		Ok(fsinfo3 {
			obj_attributes,
			rtmax: READ_MAX,
			rtpref: READ_MAX,
			rtmult: 4096,
			wtmax: READ_MAX,
			wtpref: READ_MAX,
			wtmult: 4096,
			dtpref: 64 * 1024,
			maxfilesize: i64::MAX as u64,
			time_delta: nfstime3 {
				seconds: 0,
				nseconds: 1,
			},
			properties: nfsserve::nfs::FSF_HOMOGENEOUS,
		})
	}

	/// Refuse every path. nfsserve's MNT is the only caller, on the internal
	/// port, where any local user can connect and read its export name with
	/// EXPORT; the front answers MOUNT itself and never needs this.
	async fn path_to_id(&self, _path: &[u8]) -> Result<fileid3, nfsstat3> {
		Err(nfsstat3::NFS3ERR_ACCES)
	}

	/// The file id followed by a tag keyed with this server's secret. A
	/// client that never got the handle from a MOUNT or LOOKUP reply cannot
	/// make one, so knowing the root is always 1 is not enough.
	fn id_to_fh(&self, id: fileid3) -> nfs_fh3 {
		let mut data = id.to_le_bytes().to_vec();
		data.extend_from_slice(&self.handle_tag(id));
		nfs_fh3 { data }
	}

	fn fh_to_id(&self, fh: &nfs_fh3) -> Result<fileid3, nfsstat3> {
		let (id, tag) = fh
			.data
			.split_first_chunk::<8>()
			.filter(|(_, tag)| tag.len() == HANDLE_TAG)
			.ok_or(nfsstat3::NFS3ERR_BADHANDLE)?;
		let id = u64::from_le_bytes(*id);
		let expected = self.handle_tag(id);
		// Compare every byte so the time taken says nothing about the tag.
		let diff = expected
			.iter()
			.zip(tag)
			.fold(0u8, |diff, (a, b)| diff | (a ^ b));
		if diff != 0 {
			// Also what a client sees after the daemon restarts with a new key.
			return Err(nfsstat3::NFS3ERR_STALE);
		}
		Ok(id)
	}

	fn serverid(&self) -> cookieverf3 {
		let started = self
			.inner
			.started
			.duration_since(UNIX_EPOCH)
			.unwrap_or_default();
		(started.as_nanos() as u64).to_le_bytes()
	}
}

/// A running NFS server. Dropping it stops serving.
pub struct NfsServer {
	addr: SocketAddr,
	backend: SocketAddr,
	adapter: NfsAdapter,
	export: Arc<Export>,
	stop: watch::Sender<bool>,
	tasks: Vec<JoinHandle<()>>,
}

impl NfsServer {
	/// The loopback port both the NFS and MOUNT programs answer on.
	pub fn port(&self) -> u16 {
		self.addr.port()
	}

	pub fn addr(&self) -> SocketAddr {
		self.addr
	}

	/// The internal port nfsserve answers on behind the front. Any local
	/// user can reach it too, which the security tests check.
	#[doc(hidden)]
	pub fn backend_addr(&self) -> SocketAddr {
		self.backend
	}

	/// The secret export path a client must MOUNT, once. Never log it.
	pub fn export_path(&self) -> &str {
		&self.export.path
	}

	/// Refuse every call from now on while the server keeps running, for a
	/// mount the OS would not let go of.
	pub fn close(&self) {
		self.adapter.close();
	}

	/// Stop accepting connections, close the open ones, and refuse every
	/// call still in flight. Unmount first: a client that still has the
	/// mount gets I/O errors.
	pub fn shutdown(self) {}
}

impl Drop for NfsServer {
	fn drop(&mut self) {
		self.adapter.close();
		let _ = self.stop.send(true);
		for task in &self.tasks {
			task.abort();
		}
	}
}

/// Serve `fs` over NFSv3 on 127.0.0.1, on a port the OS picks. The export
/// is a random path (see [`NfsServer::export_path`]), and the MOUNT program
/// answers on the same port, so a client needs no portmapper.
pub async fn serve(fs: Arc<dyn MountFs>) -> std::io::Result<NfsServer> {
	let changes = fs.changes();
	let adapter = NfsAdapter::new(fs.clone());
	let export = Arc::new(Export::new());
	// nfsserve answers on an internal port; the front owns the public one
	// and answers MOUNT itself. The internal port is reachable by any local
	// user, so the adapter refuses nfsserve's own MNT whatever path it names.
	let backend =
		NFSTcpListener::bind(&format!("{}:0", Ipv4Addr::LOCALHOST), adapter.clone()).await?;
	let backend_addr = SocketAddr::from((backend.get_listen_ip(), backend.get_listen_port()));
	let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
	let addr = listener.local_addr()?;
	let (stop, stopped) = watch::channel(false);
	let mut tasks = vec![
		tokio::spawn(async move {
			if let Err(err) = backend.handle_forever().await {
				tracing::warn!(error = %err, "NFS server stopped accepting");
			}
		}),
		tokio::spawn(super::front::run(
			listener,
			backend_addr,
			adapter.clone(),
			export.clone(),
			stopped,
		)),
	];
	if let Some(mut changes) = changes {
		let adapter = adapter.clone();
		tasks.push(tokio::spawn(async move {
			use tokio::sync::broadcast::error::RecvError;
			loop {
				let path = match changes.recv().await {
					Ok(path) => path,
					// A missed change could be anything, so everything goes.
					Err(RecvError::Lagged(_)) => PathBuf::new(),
					Err(RecvError::Closed) => break,
				};
				adapter.invalidate(&path);
				// NFSv3 cannot call a client back, so the server's own
				// caches are all there is to drop; the client re-asks once
				// its one-second attribute cache runs out.
				fs.invalidated(&path);
			}
		}));
	}
	info!(%addr, "NFS server up");
	Ok(NfsServer {
		addr,
		backend: backend_addr,
		adapter,
		export,
		stop,
		tasks,
	})
}

/// Mount options for this OS's NFS client. Attributes are cached for one
/// second, as the FUSE mount does for local sources, and a soft mount
/// returns an error instead of hanging if the server goes away.
#[cfg(target_os = "macos")]
fn options(port: u16) -> String {
	// rdirplus lists directories with attributes in one call, so Finder and
	// ls -l do not stat each entry separately.
	format!("vers=3,tcp,port={port},mountport={port},locallocks,rdonly,soft,rdirplus,actimeo=1")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn options(port: u16) -> String {
	format!(
		"vers=3,tcp,port={port},mountport={port},mountproto=tcp,nolock,ro,soft,timeo=50,retrans=2,actimeo=1"
	)
}

/// Whether this process runs as root.
#[cfg(unix)]
fn root() -> bool {
	unsafe { libc::geteuid() == 0 }
}

/// A command that runs as root on Linux, where mounting NFS needs it. It
/// goes through `sudo -n`, so it fails instead of prompting.
#[cfg(unix)]
fn privileged(program: &str) -> tokio::process::Command {
	if cfg!(target_os = "macos") || root() {
		return tokio::process::Command::new(program);
	}
	let mut command = tokio::process::Command::new("sudo");
	command.arg("-n").arg(program);
	command
}

/// Run `command`, naming it by `program` alone on failure: the arguments
/// of a mount carry the secret export path.
#[cfg(unix)]
async fn run(program: &str, mut command: tokio::process::Command) -> std::io::Result<()> {
	let out = command.output().await?;
	if out.status.success() {
		return Ok(());
	}
	Err(std::io::Error::other(format!(
		"{program} failed: {}",
		String::from_utf8_lossy(&out.stderr).trim()
	)))
}

/// Ask the OS's NFS client to mount `server` at `dir`, read-only.
///
/// On macOS this runs `/sbin/mount_nfs` as the current user, who must own
/// `dir`. On Linux `mount -t nfs` needs root, so it runs through
/// `sudo -n` when the process is not root; Linux has FUSE, so this path
/// exists for tests.
#[cfg(unix)]
pub async fn mount(server: &NfsServer, dir: &Path) -> std::io::Result<()> {
	let source = format!("{}:{}", Ipv4Addr::LOCALHOST, server.export_path());
	let options = options(server.port());
	let (program, mut command) = if cfg!(target_os = "macos") {
		("mount_nfs", tokio::process::Command::new("/sbin/mount_nfs"))
	} else {
		let mut command = privileged("mount");
		command.arg("-t").arg("nfs");
		("mount", command)
	};
	command.arg("-o").arg(&options).arg(&source).arg(dir);
	run(program, command).await?;
	info!(dir = %dir.display(), port = server.port(), "NFS mount up");
	Ok(())
}

/// Detach the NFS mount at `dir`. A busy mount is forced off, so a program
/// still holding a file in it gets errors rather than keeping it alive.
#[cfg(unix)]
pub async fn unmount(dir: &Path) -> std::io::Result<()> {
	let mut plain = privileged("umount");
	plain.arg(dir);
	let Err(err) = run("umount", plain).await else {
		return Ok(());
	};
	debug!(dir = %dir.display(), error = %err, "unmount refused; forcing");
	let mut forced = privileged("umount");
	if cfg!(target_os = "macos") {
		forced.arg("-f");
	} else {
		forced.arg("-f").arg("-l");
	}
	forced.arg(dir);
	run("umount", forced).await
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_path_keeps_its_id() {
		let mut ids = Ids::new();
		let a = ids.id(Path::new("docs/a.txt"));
		assert_eq!(ids.id(Path::new("docs/a.txt")), a);
		assert_ne!(ids.id(Path::new("docs/b.txt")), a);
		assert_eq!(ids.path(a), Some(PathBuf::from("docs/a.txt")));
		assert_eq!(ids.id(Path::new("")), ROOT);
		assert!(a > ROOT);
	}
}
