//! Acceptance for the read-only NFS mount (`docs/core/nfs-mount.mdx`).
//!
//! The protocol tests run everywhere the `nfs` feature builds. One drives the
//! NFS adapter directly over an in-memory tree: lookups, attributes, paged
//! listings, ranged reads, refused writes, missing and denied paths, and the
//! open-file cache. The other binds the server and speaks ONC RPC to it over
//! TCP, the way a kernel would, for NULL, MNT, LOOKUP, GETATTR, READ and a
//! paged READDIR.
//!
//! The kernel test mounts a running daemon's sources through the ops the CLI
//! calls: it lists, stats and reads a tracked source, lists a directory too
//! large for one reply, is refused writes, narrows a second mount to a grant
//! and watches it refuse and then go away once the grant is revoked, and
//! checks that shutdown unmounts what is left. It needs the OS's NFS client:
//! `/sbin/mount_nfs` on macOS, `mount.nfs` and root or passwordless sudo on
//! Linux. Without them it skips with the reason, unless `SD_REQUIRE_NFS` is
//! set, in which case it fails.

#![cfg(feature = "nfs")]

mod helpers;

use std::collections::{BTreeMap, HashSet};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use bytes::Bytes;
use nfsserve::nfs::{fileid3, nfsstat3, sattr3};
use nfsserve::vfs::NFSFileSystem;
use sd_core::service::mounts::{
	nfs::server::{self, NfsAdapter},
	provider::{ByteError, ByteProvider, ByteStat, ByteTarget, ProviderClass},
	vfs::{DirEntry, MountError, MountFs, NodeAttr, NodeKind, OpenedFile},
};
use tokio::sync::broadcast;
use uuid::Uuid;

/// A tree held in memory. Anything under `secret` is listed and stat'ed but
/// refused on open and list, the way a grant refuses a path.
struct MemFs {
	/// Each file's bytes and the time they were written.
	files: std::sync::Mutex<BTreeMap<PathBuf, (Bytes, SystemTime)>>,
	dirs: HashSet<PathBuf>,
	opens: AtomicUsize,
	changes: broadcast::Sender<PathBuf>,
}

impl MemFs {
	fn new(files: &[(&str, Bytes)]) -> Self {
		let mut dirs = HashSet::from([PathBuf::new()]);
		for (path, _) in files {
			let mut parent = Path::new(path).parent();
			while let Some(dir) = parent {
				dirs.insert(dir.to_path_buf());
				parent = dir.parent();
			}
		}
		Self {
			files: std::sync::Mutex::new(
				files
					.iter()
					.map(|(path, bytes)| (PathBuf::from(path), (bytes.clone(), UNIX_EPOCH)))
					.collect(),
			),
			dirs,
			opens: AtomicUsize::new(0),
			changes: broadcast::channel(16).0,
		}
	}

	/// Replace a file's bytes, as an atomic rename at the origin would.
	fn replace(&self, path: &str, bytes: &'static [u8], modified: SystemTime) {
		self.files
			.lock()
			.unwrap()
			.insert(PathBuf::from(path), (Bytes::from_static(bytes), modified));
	}

	fn file(&self, path: &Path) -> Option<(Bytes, SystemTime)> {
		self.files.lock().unwrap().get(path).cloned()
	}

	fn denied(path: &Path) -> bool {
		path.starts_with("secret")
	}
}

struct MemProvider(Bytes);

#[async_trait]
impl ByteProvider for MemProvider {
	async fn stat(&self, _target: &ByteTarget) -> Result<ByteStat, ByteError> {
		Ok(ByteStat {
			size: self.0.len() as u64,
			modified: None,
			version: None,
		})
	}

	async fn read_range(
		&self,
		_target: &ByteTarget,
		range: Range<u64>,
	) -> Result<Bytes, ByteError> {
		let len = self.0.len() as u64;
		let start = range.start.min(len) as usize;
		let end = range.end.min(len) as usize;
		Ok(self.0.slice(start..end))
	}

	fn class(&self) -> ProviderClass {
		ProviderClass::Local
	}

	fn max_read(&self) -> u64 {
		// Small, so one NFS read spans several provider reads.
		4096
	}
}

#[async_trait]
impl MountFs for MemFs {
	async fn stat(&self, path: &Path) -> Result<NodeAttr, MountError> {
		if self.dirs.contains(path) {
			return Ok(NodeAttr::directory(Duration::from_secs(1)));
		}
		let (bytes, modified) = self.file(path).ok_or(MountError::NotFound)?;
		Ok(NodeAttr {
			kind: NodeKind::File,
			size: bytes.len() as u64,
			modified: Some(modified),
			accessed: None,
			created: None,
			valid_for: Duration::from_secs(1),
		})
	}

	async fn list(&self, path: &Path) -> Result<Vec<DirEntry>, MountError> {
		if self.file(path).is_some() {
			return Err(MountError::NotADirectory);
		}
		if !self.dirs.contains(path) {
			return Err(MountError::NotFound);
		}
		if Self::denied(path) {
			return Err(MountError::Denied("not granted".into()));
		}
		let mut entries: Vec<DirEntry> = self
			.dirs
			.iter()
			.filter(|dir| dir.parent() == Some(path) && !dir.as_os_str().is_empty())
			.map(|dir| DirEntry {
				name: dir.file_name().unwrap().to_os_string(),
				kind: NodeKind::Directory,
			})
			.collect();
		entries.extend(
			self.files
				.lock()
				.unwrap()
				.keys()
				.filter(|file| file.parent() == Some(path))
				.map(|file| DirEntry {
					name: file.file_name().unwrap().to_os_string(),
					kind: NodeKind::File,
				}),
		);
		Ok(entries)
	}

	async fn open(&self, path: &Path) -> Result<OpenedFile, MountError> {
		if self.dirs.contains(path) {
			return Err(MountError::IsADirectory);
		}
		let (bytes, _) = self.file(path).ok_or(MountError::NotFound)?;
		if Self::denied(path) {
			return Err(MountError::Denied("not granted".into()));
		}
		self.opens.fetch_add(1, Ordering::SeqCst);
		Ok(OpenedFile {
			provider: Arc::new(MemProvider(bytes.clone())),
			target: ByteTarget {
				source_id: Uuid::nil(),
				path: path.to_path_buf(),
			},
		})
	}

	fn changes(&self) -> Option<broadcast::Receiver<PathBuf>> {
		Some(self.changes.subscribe())
	}
}

fn pattern(len: usize) -> Bytes {
	Bytes::from((0..len).map(|i| (i % 251) as u8).collect::<Vec<u8>>())
}

fn tree() -> Arc<MemFs> {
	let mut files: Vec<(String, Bytes)> = vec![
		(
			"docs/readme.md".into(),
			Bytes::from_static(b"hello from nfs\n"),
		),
		("docs/big.bin".into(), pattern(100_000)),
		("secret/key.txt".into(), Bytes::from_static(b"secret")),
	];
	for i in 0..1000 {
		files.push((format!("many/file-{i:04}.txt"), Bytes::from(i.to_string())));
	}
	let files: Vec<(&str, Bytes)> = files.iter().map(|(p, b)| (p.as_str(), b.clone())).collect();
	Arc::new(MemFs::new(&files))
}

async fn walk(adapter: &NfsAdapter, path: &str) -> Result<fileid3, nfsstat3> {
	let mut id = adapter.root_dir();
	for name in path.split('/') {
		id = adapter.lookup(id, &name.as_bytes().into()).await?;
	}
	Ok(id)
}

#[tokio::test]
async fn the_adapter_answers_nfs_calls() {
	let fs = tree();
	let adapter = NfsAdapter::new(fs.clone());
	let root = adapter.root_dir();
	assert_eq!(root, 1);

	// Lookup and attributes.
	let docs = walk(&adapter, "docs").await.unwrap();
	let readme = walk(&adapter, "docs/readme.md").await.unwrap();
	assert_eq!(
		walk(&adapter, "docs/readme.md").await.map_err(|e| e as u32),
		Ok(readme),
		"ids are stable"
	);
	assert_eq!(
		adapter
			.lookup(readme, &b"x".as_slice().into())
			.await
			.map_err(|e| e as u32),
		Err(nfsstat3::NFS3ERR_NOENT as u32)
	);
	assert_eq!(
		adapter
			.lookup(docs, &b"..".as_slice().into())
			.await
			.map_err(|e| e as u32),
		Ok(root)
	);
	assert_eq!(
		adapter
			.lookup(docs, &b".".as_slice().into())
			.await
			.map_err(|e| e as u32),
		Ok(docs)
	);
	assert_eq!(
		walk(&adapter, "docs/absent.md").await.map_err(|e| e as u32),
		Err(nfsstat3::NFS3ERR_NOENT as u32)
	);
	let attr = adapter.getattr(readme).await.unwrap();
	assert_eq!(attr.size, 15);
	assert_eq!(attr.fileid, readme);
	assert_eq!(attr.mode, 0o444);
	assert!(matches!(attr.ftype, nfsserve::nfs::ftype3::NF3REG));
	let dir_attr = adapter.getattr(docs).await.unwrap();
	assert!(matches!(dir_attr.ftype, nfsserve::nfs::ftype3::NF3DIR));
	assert_eq!(
		adapter.getattr(0xdead_beef).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_STALE as u32)
	);

	// A listing pages by cookie, in order, without gaps or repeats.
	let many = walk(&adapter, "many").await.unwrap();
	let mut names = Vec::new();
	let mut cookie = 0;
	let mut pages = 0;
	loop {
		let page = adapter.readdir(many, cookie, 64).await.unwrap();
		pages += 1;
		assert!(page.entries.len() <= 64);
		for entry in &page.entries {
			names.push(String::from_utf8(entry.name.0.clone()).unwrap());
			assert_eq!(entry.attr.fileid, entry.fileid);
			assert_eq!(
				walk(&adapter, &format!("many/{}", names.last().unwrap()))
					.await
					.map_err(|e| e as u32),
				Ok(entry.fileid)
			);
		}
		if page.end {
			break;
		}
		cookie = page.entries.last().unwrap().fileid;
	}
	assert_eq!(pages, 1000usize.div_ceil(64));
	let expected: Vec<String> = (0..1000).map(|i| format!("file-{i:04}.txt")).collect();
	assert_eq!(names, expected);
	assert_eq!(
		adapter
			.readdir(many, 0x1234_5678, 10)
			.await
			.err()
			.map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_BAD_COOKIE as u32)
	);
	assert_eq!(
		adapter.readdir(readme, 0, 10).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_NOTDIR as u32)
	);

	// Reads honor offset and count and flag the end.
	let (bytes, eof) = adapter.read(readme, 0, 4096).await.unwrap();
	assert_eq!(bytes, b"hello from nfs\n");
	assert!(eof);
	let (bytes, eof) = adapter.read(readme, 6, 4).await.unwrap();
	assert_eq!(bytes, b"from");
	assert!(!eof);
	let (bytes, eof) = adapter.read(readme, 11, 4).await.unwrap();
	assert_eq!(bytes, b"nfs\n");
	assert!(eof, "a read that ends at the end of the file is the last");
	let (bytes, eof) = adapter.read(readme, 100, 10).await.unwrap();
	assert!(bytes.is_empty() && eof);
	let big = walk(&adapter, "docs/big.bin").await.unwrap();
	let expected = pattern(100_000);
	let mut offset = 0u64;
	loop {
		let (bytes, eof) = adapter.read(big, offset, 32 * 1024).await.unwrap();
		assert_eq!(
			bytes,
			expected[offset as usize..offset as usize + bytes.len()]
		);
		offset += bytes.len() as u64;
		if eof {
			break;
		}
	}
	assert_eq!(offset, 100_000);
	assert_eq!(
		fs.opens.load(Ordering::SeqCst),
		2,
		"sequential reads reuse the opened file"
	);
	assert_eq!(
		adapter.read(docs, 0, 10).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ISDIR as u32)
	);

	// The change feed drops the cached open.
	adapter.invalidate(Path::new("docs/readme.md"));
	adapter.read(readme, 0, 4).await.unwrap();
	assert_eq!(fs.opens.load(Ordering::SeqCst), 3);

	// A file replaced at its origin is reopened, even at the same size and
	// with no change feed, so new bytes never arrive under old attributes.
	fs.replace(
		"docs/readme.md",
		b"HELLO FROM NFS\n",
		UNIX_EPOCH + Duration::from_secs(60),
	);
	let (bytes, _) = adapter.read(readme, 0, 4096).await.unwrap();
	assert_eq!(bytes, b"HELLO FROM NFS\n");
	assert_eq!(fs.opens.load(Ordering::SeqCst), 4);
	assert_eq!(adapter.getattr(readme).await.unwrap().mtime.seconds, 60);

	// A directory with no time of its own reports now, so a client that
	// cached its listing asks again.
	let now = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap()
		.as_secs() as u32;
	let root_attr = adapter.getattr(root).await.unwrap();
	assert!(root_attr.mtime.seconds + 2 >= now, "{:?}", root_attr.mtime);

	// Denied paths are refused with EACCES.
	let secret = walk(&adapter, "secret").await.unwrap();
	assert_eq!(
		adapter.readdir(secret, 0, 10).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ACCES as u32)
	);
	let key = walk(&adapter, "secret/key.txt").await.unwrap();
	assert_eq!(
		adapter.read(key, 0, 10).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ACCES as u32)
	);

	// Every mutation is refused as a read-only filesystem.
	let name = b"new.txt".as_slice().into();
	assert_eq!(
		adapter.write(readme, 0, b"x").await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter
			.setattr(readme, sattr3::default())
			.await
			.err()
			.map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter
			.create(docs, &name, sattr3::default())
			.await
			.err()
			.map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter
			.create_exclusive(docs, &name)
			.await
			.err()
			.map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter.mkdir(docs, &name).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter
			.remove(docs, &b"readme.md".as_slice().into())
			.await
			.err()
			.map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter
			.rename(docs, &b"readme.md".as_slice().into(), docs, &name)
			.await
			.err()
			.map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter
			.symlink(
				docs,
				&name,
				&b"readme.md".as_slice().into(),
				&sattr3::default()
			)
			.await
			.err()
			.map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_ROFS as u32)
	);
	assert_eq!(
		adapter.readlink(readme).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_INVAL as u32)
	);
	assert!(matches!(
		adapter.capabilities(),
		nfsserve::vfs::VFSCapabilities::ReadOnly
	));

	// A closed adapter refuses everything.
	adapter.close();
	assert_eq!(
		adapter.getattr(readme).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_IO as u32)
	);
	assert_eq!(
		adapter.read(readme, 0, 4).await.err().map(|e| e as u32),
		Some(nfsstat3::NFS3ERR_IO as u32)
	);
}

/// Just enough ONC RPC over TCP to act as a client.
struct RpcClient {
	stream: TcpStream,
	xid: u32,
}

const NFS_PROGRAM: u32 = 100003;
const MOUNT_PROGRAM: u32 = 100005;

fn put_u32(buf: &mut Vec<u8>, value: u32) {
	buf.extend_from_slice(&value.to_be_bytes());
}

fn put_u64(buf: &mut Vec<u8>, value: u64) {
	buf.extend_from_slice(&value.to_be_bytes());
}

fn put_opaque(buf: &mut Vec<u8>, data: &[u8]) {
	put_u32(buf, data.len() as u32);
	buf.extend_from_slice(data);
	buf.resize(buf.len() + (4 - data.len() % 4) % 4, 0);
}

struct Reply {
	data: Vec<u8>,
	at: usize,
}

impl Reply {
	fn u32(&mut self) -> u32 {
		let value = u32::from_be_bytes(self.data[self.at..self.at + 4].try_into().unwrap());
		self.at += 4;
		value
	}

	fn u64(&mut self) -> u64 {
		let value = u64::from_be_bytes(self.data[self.at..self.at + 8].try_into().unwrap());
		self.at += 8;
		value
	}

	fn opaque(&mut self) -> Vec<u8> {
		let len = self.u32() as usize;
		let data = self.data[self.at..self.at + len].to_vec();
		self.at += len + (4 - len % 4) % 4;
		data
	}

	/// A fattr3, returning (type, size, fileid).
	fn fattr(&mut self) -> (u32, u64, u64) {
		let ftype = self.u32();
		self.at += 4 * 4;
		let size = self.u64();
		self.at += 8 * 3;
		let fileid = self.u64();
		self.at += 8 * 3;
		(ftype, size, fileid)
	}

	fn post_op_attr(&mut self) -> Option<(u32, u64, u64)> {
		(self.u32() == 1).then(|| self.fattr())
	}
}

impl RpcClient {
	fn call(&mut self, program: u32, procedure: u32, args: &[u8]) -> Reply {
		self.xid += 1;
		let mut body = Vec::new();
		put_u32(&mut body, self.xid);
		put_u32(&mut body, 0); // CALL
		put_u32(&mut body, 2); // RPC version
		put_u32(&mut body, program);
		put_u32(&mut body, 3);
		put_u32(&mut body, procedure);
		// AUTH_UNIX credentials, as kernels send them.
		let mut cred = Vec::new();
		put_u32(&mut cred, 0);
		put_opaque(&mut cred, b"test");
		put_u32(&mut cred, 0);
		put_u32(&mut cred, 0);
		put_u32(&mut cred, 0);
		put_u32(&mut body, 1);
		put_opaque(&mut body, &cred);
		put_u32(&mut body, 0);
		put_u32(&mut body, 0);
		body.extend_from_slice(args);
		let mut record = Vec::new();
		put_u32(&mut record, 0x8000_0000 | body.len() as u32);
		record.extend_from_slice(&body);
		self.stream.write_all(&record).unwrap();

		let mut data = Vec::new();
		loop {
			let mut mark = [0u8; 4];
			self.stream.read_exact(&mut mark).unwrap();
			let mark = u32::from_be_bytes(mark);
			let mut fragment = vec![0u8; (mark & 0x7fff_ffff) as usize];
			self.stream.read_exact(&mut fragment).unwrap();
			data.extend_from_slice(&fragment);
			if mark & 0x8000_0000 != 0 {
				break;
			}
		}
		let mut reply = Reply { data, at: 0 };
		assert_eq!(reply.u32(), self.xid);
		assert_eq!(reply.u32(), 1, "a reply");
		assert_eq!(reply.u32(), 0, "accepted");
		reply.u32();
		reply.opaque();
		assert_eq!(reply.u32(), 0, "the call succeeded at the RPC level");
		reply
	}
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_server_speaks_nfs_and_mount_over_tcp() {
	let nfs = server::serve(tree()).await.unwrap();
	assert!(nfs.addr().ip().is_loopback());
	let addr = nfs.addr();
	let export = nfs.export_path().to_string();
	assert_eq!(export.len(), 33, "a slash and 128 bits in hex");
	tokio::task::spawn_blocking(move || {
		let stream = TcpStream::connect(addr).unwrap();
		stream
			.set_read_timeout(Some(Duration::from_secs(10)))
			.unwrap();
		let mut client = RpcClient { stream, xid: 0 };

		// NULL on both programs.
		let reply = client.call(NFS_PROGRAM, 0, &[]);
		assert_eq!(reply.at, reply.data.len());
		client.call(MOUNT_PROGRAM, 0, &[]);

		// Nothing on the port says what to mount.
		let mut reply = client.call(MOUNT_PROGRAM, 5, &[]);
		assert_eq!(reply.u32(), 0, "EXPORT lists no exports");
		let mut reply = client.call(MOUNT_PROGRAM, 2, &[]);
		assert_eq!(reply.u32(), 0, "DUMP lists no mounts");

		// MNT for anything but the secret export is refused.
		let mnt = |client: &mut RpcClient, path: &[u8]| {
			let mut args = Vec::new();
			put_opaque(&mut args, path);
			let mut reply = client.call(MOUNT_PROGRAM, 1, &args);
			match reply.u32() {
				0 => Ok(reply.opaque()),
				status => Err(status),
			}
		};
		let truncated = &export.as_bytes()[..export.len() - 1];
		for wrong in [&b"/"[..], b"/elsewhere", truncated] {
			assert_eq!(mnt(&mut client, wrong), Err(13), "MNT3ERR_ACCES");
		}
		let root = mnt(&mut client, export.as_bytes()).expect("MNT3_OK");
		assert_eq!(
			mnt(&mut client, export.as_bytes()),
			Err(13),
			"the export is handed out once"
		);

		let lookup = |client: &mut RpcClient, dir: &[u8], name: &[u8]| {
			let mut args = Vec::new();
			put_opaque(&mut args, dir);
			put_opaque(&mut args, name);
			let mut reply = client.call(NFS_PROGRAM, 3, &args);
			match reply.u32() {
				0 => {
					let handle = reply.opaque();
					let attr = reply.post_op_attr().unwrap();
					Ok((handle, attr))
				}
				status => Err(status),
			}
		};
		let (docs, (ftype, _, _)) = lookup(&mut client, &root, b"docs").unwrap();
		assert_eq!(ftype, 2, "a directory");
		let (readme, (ftype, size, readme_id)) = lookup(&mut client, &docs, b"readme.md").unwrap();
		assert_eq!((ftype, size), (1, 15));
		assert_eq!(
			lookup(&mut client, &docs, b"absent").err(),
			Some(2),
			"NFS3ERR_NOENT"
		);

		// GETATTR.
		let getattr = |client: &mut RpcClient, handle: &[u8]| {
			let mut args = Vec::new();
			put_opaque(&mut args, handle);
			let mut reply = client.call(NFS_PROGRAM, 1, &args);
			match reply.u32() {
				0 => Ok(reply.fattr()),
				status => Err(status),
			}
		};
		assert_eq!(getattr(&mut client, &readme), Ok((1, 15, readme_id)));

		// A handle the server did not hand out is refused, on the calls
		// nfsserve answers and on the READDIR the front answers. The root is
		// always file id 1, so the untagged handle nfsserve would make for it,
		// and the real id with a guessed tag, are what a scanner would try.
		let mut untagged = Vec::new();
		untagged.extend_from_slice(&0u64.to_le_bytes());
		untagged.extend_from_slice(&1u64.to_le_bytes());
		let mut forged = root.clone();
		*forged.last_mut().unwrap() ^= 1;
		let mut moved = root.clone();
		moved[..8].copy_from_slice(&readme_id.to_le_bytes());
		for (handle, status, what) in [
			(&untagged, 10001, "an untagged handle is NFS3ERR_BADHANDLE"),
			(&forged, 70, "a wrong tag is NFS3ERR_STALE"),
			(&moved, 70, "a tag moved to another file is NFS3ERR_STALE"),
		] {
			assert_eq!(getattr(&mut client, handle).err(), Some(status), "{what}");
			let mut args = Vec::new();
			put_opaque(&mut args, handle);
			put_u64(&mut args, 0);
			put_u64(&mut args, 0);
			put_u32(&mut args, 4096);
			let mut reply = client.call(NFS_PROGRAM, 16, &args);
			assert_eq!(reply.u32(), status, "READDIR: {what}");
		}

		// READ a range, then to the end.
		let read = |client: &mut RpcClient, offset: u64, count: u32| {
			let mut args = Vec::new();
			put_opaque(&mut args, &readme);
			put_u64(&mut args, offset);
			put_u32(&mut args, count);
			let mut reply = client.call(NFS_PROGRAM, 6, &args);
			assert_eq!(reply.u32(), 0, "NFS3_OK");
			reply.post_op_attr();
			let count = reply.u32();
			let eof = reply.u32() == 1;
			let data = reply.opaque();
			assert_eq!(count as usize, data.len());
			(data, eof)
		};
		assert_eq!(read(&mut client, 6, 4), (b"from".to_vec(), false));
		assert_eq!(
			read(&mut client, 0, 1024),
			(b"hello from nfs\n".to_vec(), true)
		);

		// Plain READDIR pages by cookie through a large directory.
		let (many, _) = lookup(&mut client, &root, b"many").unwrap();
		let mut names = Vec::new();
		let mut cookie = 0u64;
		let mut pages = 0;
		loop {
			let mut args = Vec::new();
			put_opaque(&mut args, &many);
			put_u64(&mut args, cookie);
			put_u64(&mut args, 0);
			put_u32(&mut args, 4096);
			let mut reply = client.call(NFS_PROGRAM, 16, &args);
			assert_eq!(reply.u32(), 0, "NFS3_OK");
			reply.post_op_attr();
			reply.u64();
			pages += 1;
			while reply.u32() == 1 {
				let fileid = reply.u64();
				names.push(String::from_utf8(reply.opaque()).unwrap());
				cookie = reply.u64();
				assert_eq!(cookie, fileid);
			}
			if reply.u32() == 1 {
				break;
			}
			assert!(pages < 1000, "the listing ends");
		}
		assert!(pages > 1, "the listing took more than one reply");
		let expected: Vec<String> = (0..1000).map(|i| format!("file-{i:04}.txt")).collect();
		assert_eq!(names, expected);
		let mut args = Vec::new();
		put_opaque(&mut args, &many);
		put_u64(&mut args, 0x1234_5678);
		put_u64(&mut args, 0);
		put_u32(&mut args, 4096);
		let mut reply = client.call(NFS_PROGRAM, 16, &args);
		assert_eq!(reply.u32(), 10003, "NFS3ERR_BAD_COOKIE");

		// A write is refused.
		let mut args = Vec::new();
		put_opaque(&mut args, &readme);
		put_u64(&mut args, 0);
		put_u32(&mut args, 1);
		put_u32(&mut args, 0);
		put_opaque(&mut args, b"x");
		let mut reply = client.call(NFS_PROGRAM, 7, &args);
		assert_eq!(reply.u32(), 30, "NFS3ERR_ROFS");
	})
	.await
	.unwrap();
	nfs.shutdown();
}

/// Collects everything a test's subscriber writes.
#[derive(Clone, Default)]
struct Captured(Arc<std::sync::Mutex<Vec<u8>>>);

impl Write for Captured {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		self.0.lock().unwrap().extend_from_slice(buf);
		Ok(buf.len())
	}

	fn flush(&mut self) -> std::io::Result<()> {
		Ok(())
	}
}

/// The export path is the only thing that lets a local client MOUNT, so it
/// must not reach a log at any level, nfsserve's included.
#[test]
fn the_export_path_is_never_logged() {
	let logs = Captured::default();
	let subscriber = tracing_subscriber::fmt()
		.with_max_level(tracing::Level::TRACE)
		.with_ansi(false)
		.with_writer({
			let logs = logs.clone();
			move || logs.clone()
		})
		.finish();
	// A current-thread runtime keeps every server task under this subscriber.
	let export = tracing::subscriber::with_default(subscriber, || {
		let runtime = tokio::runtime::Builder::new_current_thread()
			.enable_all()
			.build()
			.unwrap();
		runtime.block_on(async {
			let nfs = server::serve(tree()).await.unwrap();
			let export = nfs.export_path().to_string();
			let addr = nfs.addr();
			let path = export.clone();
			tokio::task::spawn_blocking(move || {
				let stream = TcpStream::connect(addr).unwrap();
				stream
					.set_read_timeout(Some(Duration::from_secs(10)))
					.unwrap();
				let mut client = RpcClient { stream, xid: 0 };
				for attempt in [path.as_bytes(), path.as_bytes(), b"/"] {
					let mut args = Vec::new();
					put_opaque(&mut args, attempt);
					client.call(MOUNT_PROGRAM, 1, &args);
				}
				client.call(NFS_PROGRAM, 0, &[]);
			})
			.await
			.unwrap();
			nfs.shutdown();
			export
		})
	});
	let logs = String::from_utf8(logs.0.lock().unwrap().clone()).unwrap();
	assert!(
		logs.contains("NFS server up"),
		"the subscriber saw the server"
	);
	let token = export.trim_start_matches('/');
	assert!(!logs.contains(token), "the export path was logged");
}

#[cfg(unix)]
mod kernel {
	use super::helpers::*;
	use sd_core::{
		infra::action::{CoreAction, LibraryAction},
		ops::{
			config::app::{UpdateAppConfigAction, UpdateAppConfigInput},
			grants::{
				create::{CreateGrantAction, CreateGrantInput},
				revoke::{RevokeGrantAction, RevokeGrantInput},
				GrantCap,
			},
			mounts::{NfsMountAction, NfsMountInput, NfsUnmountAction, NfsUnmountInput},
		},
		service::mounts::nfs,
	};
	use std::io::{Read, Seek, SeekFrom};
	use std::path::Path;
	use std::time::Duration;
	use tracing::warn;

	const EROFS: i32 = 30;
	const EACCES: i32 = 13;
	const ENOENT: i32 = 2;
	const EIO: i32 = 5;

	fn command_works(program: &str, args: &[&str]) -> bool {
		std::process::Command::new(program)
			.args(args)
			.output()
			.is_ok_and(|out| out.status.success())
	}

	/// Why the OS's NFS client cannot mount here, if it cannot.
	fn skip_reason() -> Option<String> {
		let reason = if cfg!(target_os = "macos") {
			if Path::new("/sbin/mount_nfs").exists() {
				return None;
			}
			"/sbin/mount_nfs is missing".to_string()
		} else if !Path::new("/sbin/mount.nfs").exists()
			&& !Path::new("/usr/sbin/mount.nfs").exists()
		{
			"mount.nfs is not installed (apt install nfs-common)".to_string()
		} else if unsafe { libc::geteuid() } != 0 && !command_works("sudo", &["-n", "true"]) {
			"mounting NFS on Linux needs root or passwordless sudo".to_string()
		} else {
			return None;
		};
		if std::env::var_os("SD_REQUIRE_NFS").is_some() {
			panic!("SD_REQUIRE_NFS is set but {reason}");
		}
		Some(reason)
	}

	/// Why mounting fails here, found by mounting a small tree once. macOS
	/// may refuse an unprivileged mount_nfs on some machines.
	async fn probe() -> Option<String> {
		let dir = tempfile::tempdir().ok()?;
		let dir = std::fs::canonicalize(dir.path()).ok()?;
		let server = super::server::serve(super::tree()).await.ok()?;
		let reason = match super::server::mount(&server, &dir).await {
			Ok(()) => {
				let listed = {
					let dir = dir.clone();
					tokio::task::spawn_blocking(move || {
						std::fs::read_dir(&dir).map(|entries| entries.count())
					})
					.await
					.ok()?
				};
				let _ = super::server::unmount(&dir).await;
				match listed {
					Ok(count) if count > 0 => return None,
					other => format!("a probe mount listed nothing: {other:?}"),
				}
			}
			Err(err) => format!("a probe mount failed: {err}"),
		};
		if std::env::var_os("SD_REQUIRE_NFS").is_some() {
			panic!("SD_REQUIRE_NFS is set but {reason}");
		}
		Some(reason)
	}

	/// Whether the kernel lists a mount at `path`.
	fn kernel_mounted(path: &Path) -> bool {
		let table = if cfg!(target_os = "linux") {
			std::fs::read_to_string("/proc/self/mounts").unwrap_or_default()
		} else {
			std::process::Command::new("/sbin/mount")
				.output()
				.map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
				.unwrap_or_default()
		};
		let path = path.to_string_lossy();
		table
			.lines()
			.any(|line| line.split_whitespace().any(|field| field == path))
	}

	async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
		tokio::task::spawn_blocking(f).await.expect("blocking task")
	}

	fn names(dir: &Path) -> Vec<String> {
		let mut names: Vec<String> = std::fs::read_dir(dir)
			.unwrap_or_else(|err| panic!("list {}: {err}", dir.display()))
			.map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
			.collect();
		names.sort();
		names
	}

	fn os_error<T: std::fmt::Debug>(result: std::io::Result<T>) -> Option<i32> {
		result.expect_err("expected an error").raw_os_error()
	}

	#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
	async fn a_daemon_mounts_lists_reads_and_unmounts_over_nfs() -> anyhow::Result<()> {
		let _ = tracing_subscriber::fmt::try_init();
		let reason = match skip_reason() {
			Some(reason) => Some(reason),
			None => probe().await,
		};
		if let Some(reason) = reason {
			warn!("Skipping: {reason}");
			return Ok(());
		}

		let harness = IndexingHarnessBuilder::new("nfs_mount")
			.disable_watcher()
			.build()
			.await?;
		let tracked = harness.create_test_dir("tracked").await?;
		tracked
			.write_file("docs/readme.md", "hello from the mount\n")
			.await?;
		tracked
			.write_file("docs/deep/notes.txt", "0123456789")
			.await?;
		tracked.write_file("private/key.txt", "secret").await?;
		const MANY: usize = 3000;
		for i in 0..MANY {
			tracked
				.write_file(&format!("many/a-rather-long-file-name-{i:05}.txt"), "x")
				.await?;
		}
		let base = std::fs::canonicalize(harness.temp_path())?;
		std::os::unix::fs::symlink(
			base.join("tracked/private/key.txt"),
			base.join("tracked/docs/key-link"),
		)?;
		tracked.track().await?;
		let context = harness.core.context.clone();
		let all = base.join("mnt-all");
		let granted = base.join("mnt-grant");
		let at_shutdown = base.join("mnt-shutdown");
		for dir in [&all, &granted, &at_shutdown] {
			std::fs::create_dir(dir)?;
		}

		let mount = |path: &Path, grant_token: Option<String>| {
			NfsMountAction::from_input(NfsMountInput {
				path: path.to_path_buf(),
				grant_token,
			})
			.unwrap()
			.execute(context.clone())
		};

		// Off until the config allows it.
		let refused = mount(&all, None)
			.await
			.expect_err("mounts are off by default");
		assert!(refused.to_string().contains("nfs_enabled"), "{refused}");
		UpdateAppConfigAction::from_input(UpdateAppConfigInput {
			mounts_nfs_enabled: Some(true),
			..Default::default()
		})
		.map_err(anyhow::Error::msg)?
		.execute(context.clone())
		.await?;

		// A mount point must be an empty directory.
		assert!(mount(&base.join("tracked"), None).await.is_err());

		let info = mount(&all, None).await?;
		assert_eq!(info.path, all);
		assert!(info.port > 0);
		assert!(info.grant_id.is_none());
		assert!(kernel_mounted(&all));
		assert!(mount(&all, None).await.is_err(), "one mount per path");

		{
			let all = all.clone();
			blocking(move || {
				let shares = names(&all);
				let share = shares
					.iter()
					.find(|name| name.starts_with("tracked-"))
					.unwrap_or_else(|| panic!("the tracked source is listed: {shares:?}"));
				let root = all.join(share);
				assert_eq!(names(&root), ["docs", "many", "private"]);
				assert_eq!(
					names(&root.join("docs")),
					["deep", "readme.md"],
					"symlinks are left out, since their targets resolve outside the mount"
				);
				let many = names(&root.join("many"));
				assert_eq!(many.len(), MANY, "a large directory lists every entry once");
				let expected: Vec<String> = (0..MANY)
					.map(|i| format!("a-rather-long-file-name-{i:05}.txt"))
					.collect();
				assert_eq!(many, expected);

				let readme = root.join("docs/readme.md");
				let meta = std::fs::metadata(&readme).unwrap();
				assert!(meta.is_file());
				assert_eq!(meta.len(), "hello from the mount\n".len() as u64);
				assert!(meta.permissions().readonly());
				assert!(std::fs::metadata(root.join("docs/deep")).unwrap().is_dir());
				assert_eq!(
					std::fs::read_to_string(&readme).unwrap(),
					"hello from the mount\n"
				);

				let mut notes = std::fs::File::open(root.join("docs/deep/notes.txt")).unwrap();
				notes.seek(SeekFrom::Start(4)).unwrap();
				let mut tail = String::new();
				notes.read_to_string(&mut tail).unwrap();
				assert_eq!(tail, "456789", "reads start where they are asked to");

				assert_eq!(
					std::fs::metadata(root.join("docs/absent.md"))
						.unwrap_err()
						.kind(),
					std::io::ErrorKind::NotFound
				);
				// The NFS client checks the mode bits before the read-only
				// flag, so a create or an open for writing can fail either way.
				let refused = |result: Option<i32>| matches!(result, Some(EROFS) | Some(EACCES));
				let created = os_error(std::fs::write(root.join("docs/new.md"), "x"));
				assert!(refused(created), "create: {created:?}");
				let appended = os_error(std::fs::OpenOptions::new().append(true).open(&readme));
				assert!(refused(appended), "append: {appended:?}");
				let removed = os_error(std::fs::remove_file(&readme));
				assert!(refused(removed), "remove: {removed:?}");
				assert!(!root.join("docs/new.md").exists());
				assert_eq!(
					std::fs::read_to_string(&readme).unwrap(),
					"hello from the mount\n"
				);
			})
			.await;
		}

		// A source added after the first listing shows up at the top of the
		// mount once the client's one-second attribute cache runs out.
		let later = harness.create_test_dir("later").await?;
		later.write_file("hello.txt", "later").await?;
		later.track().await?;
		let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
		loop {
			let listed = {
				let all = all.clone();
				blocking(move || names(&all)).await
			};
			if listed.iter().any(|name| name.starts_with("later-")) {
				break;
			}
			assert!(
				tokio::time::Instant::now() < deadline,
				"a new source appears in a listed mount: {listed:?}"
			);
			tokio::time::sleep(Duration::from_millis(500)).await;
		}

		// A grant narrows a mount to the directory it lends.
		let grant = CreateGrantAction::from_input(CreateGrantInput {
			path: base.join("tracked/docs"),
			caps: Some(GrantCap::Read),
			ttl_seconds: None,
			label: Some("nfs".to_string()),
			kind: None,
			token_format: None,
		})
		.map_err(anyhow::Error::msg)?
		.execute(harness.library.clone(), context.clone())
		.await?;
		assert!(mount(&granted, Some("sdg_not-a-token".to_string()))
			.await
			.is_err());
		let info = mount(&granted, Some(grant.token.clone())).await?;
		assert_eq!(info.grant_id, Some(grant.grant.id));
		assert_eq!(info.granted_path, Some(base.join("tracked/docs")));
		{
			let granted = granted.clone();
			blocking(move || {
				assert_eq!(names(&granted), ["deep", "readme.md"]);
				assert_eq!(
					std::fs::read_to_string(granted.join("deep/notes.txt")).unwrap(),
					"0123456789"
				);
				assert_eq!(
					std::fs::metadata(granted.join("private"))
						.unwrap_err()
						.kind(),
					std::io::ErrorKind::NotFound,
					"nothing outside the granted directory is reachable"
				);
			})
			.await;
		}

		// Revoked: the next read is refused, and the mount goes away.
		RevokeGrantAction::from_input(RevokeGrantInput { id: grant.grant.id })
			.map_err(anyhow::Error::msg)?
			.execute(harness.library.clone(), context.clone())
			.await?;
		{
			let granted = granted.clone();
			let refused =
				blocking(move || os_error(std::fs::read(granted.join("readme.md")))).await;
			// EIO or ENOENT only if the mount was already taken down under the read.
			assert!(
				matches!(refused, Some(EACCES) | Some(EIO) | Some(ENOENT)),
				"a revoked grant is refused: {refused:?}"
			);
		}
		let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
		while nfs::list().await.iter().any(|m| m.path == granted) {
			assert!(
				tokio::time::Instant::now() < deadline,
				"the revoked grant's mount is taken down"
			);
			tokio::time::sleep(Duration::from_millis(250)).await;
		}
		assert!(!kernel_mounted(&granted));
		{
			let granted = granted.clone();
			assert!(blocking(move || names(&granted)).await.is_empty());
		}

		// Unmounting leaves the empty directory behind.
		NfsUnmountAction::from_input(NfsUnmountInput { path: all.clone() })
			.map_err(anyhow::Error::msg)?
			.execute(context.clone())
			.await?;
		assert!(!kernel_mounted(&all));
		{
			let all = all.clone();
			assert!(blocking(move || names(&all)).await.is_empty());
		}
		assert!(
			NfsUnmountAction::from_input(NfsUnmountInput { path: all.clone() })
				.unwrap()
				.execute(context.clone())
				.await
				.is_err()
		);

		// Shutdown takes down whatever is still mounted.
		mount(&at_shutdown, None).await?;
		assert!(kernel_mounted(&at_shutdown));
		harness.shutdown().await?;
		assert!(nfs::list().await.is_empty());
		assert!(
			!kernel_mounted(&at_shutdown),
			"shutdown left the mount in the kernel's table"
		);
		Ok(())
	}
}
