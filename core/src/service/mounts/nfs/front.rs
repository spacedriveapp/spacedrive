//! # NFS front port
//!
//! The public port of a mount's NFS server.
//!
//! nfsserve answers plain READDIR from the first entry of the directory on
//! every call: it never passes the client's cookie to the filesystem. A
//! client asks for the next page with the last cookie it saw, gets the first
//! page again, and lists forever. Linux switches from READDIRPLUS to READDIR
//! after the first page of a large directory, so this is not a corner case.
//! The front reads each RPC record a client sends, answers NFS READDIR from
//! the adapter's paged listing, and passes every other record to nfsserve on
//! an internal loopback port unchanged. Replies from both are whole records,
//! so they interleave safely on the client's connection.
//!
//! The front also answers the MOUNT program itself, so nfsserve never sees
//! the secret export path, which it would log at debug level. MNT hands out
//! the root handle once, for the export path only. EXPORT and DUMP list
//! nothing, so a client on the port cannot ask what to mount.

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use nfsserve::nfs::{fattr3, nfs_fh3, post_op_attr};
use nfsserve::vfs::NFSFileSystem;
use nfsserve::xdr::XDR;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tracing::debug;

use super::server::{Export, NfsAdapter};

const NFS_PROGRAM: u32 = 100003;
const NFS_VERSION: u32 = 3;
const READDIR: u32 = 16;
const MOUNT_PROGRAM: u32 = 100005;
const MOUNT_VERSION: u32 = 3;
const MOUNTPROC_NULL: u32 = 0;
const MOUNTPROC_MNT: u32 = 1;
const MOUNTPROC_DUMP: u32 = 2;
const MOUNTPROC_UMNT: u32 = 3;
const MOUNTPROC_UMNTALL: u32 = 4;
const MOUNTPROC_EXPORT: u32 = 5;
const MNT3ERR_ACCES: u32 = 13;
const AUTH_UNIX: u32 = 1;
const PROG_MISMATCH: u32 = 2;
const PROC_UNAVAIL: u32 = 3;
const GARBAGE_ARGS: u32 = 4;
/// Larger than any call a client sends: a WRITE carries at most the 1 MiB
/// the server offers, and is refused anyway.
const MAX_RECORD: usize = 4 * 1024 * 1024;
/// Bytes of a READDIR reply around its entries: status, directory
/// attributes, cookie verifier, the end of the entry list and the eof flag.
const READDIR_OVERHEAD: usize = 4 + 4 + 84 + 8 + 4 + 4;
const NFS3ERR_TOOSMALL: u32 = 10005;

/// Accept clients on `listener` until `stop` changes.
pub(super) async fn run(
	listener: TcpListener,
	backend: SocketAddr,
	adapter: NfsAdapter,
	export: Arc<Export>,
	mut stop: watch::Receiver<bool>,
) {
	loop {
		tokio::select! {
			accepted = listener.accept() => {
				let Ok((client, _)) = accepted else {
					continue;
				};
				let adapter = adapter.clone();
				let export = export.clone();
				let stop = stop.clone();
				tokio::spawn(async move {
					if let Err(err) = connection(client, backend, adapter, export, stop).await {
						debug!(error = %err, "NFS connection closed");
					}
				});
			}
			_ = stop.changed() => break,
		}
	}
}

async fn connection(
	client: TcpStream,
	backend: SocketAddr,
	adapter: NfsAdapter,
	export: Arc<Export>,
	mut stop: watch::Receiver<bool>,
) -> io::Result<()> {
	let server = TcpStream::connect(backend).await?;
	let _ = client.set_nodelay(true);
	let _ = server.set_nodelay(true);
	let (mut client_read, mut client_write) = client.into_split();
	let (mut server_read, mut server_write) = server.into_split();
	let (replies, mut outbox) = mpsc::channel::<Vec<u8>>(64);

	let writer = async move {
		while let Some(record) = outbox.recv().await {
			write_record(&mut client_write, &record).await?;
		}
		Ok::<(), io::Error>(())
	};
	let from_server = {
		let replies = replies.clone();
		async move {
			loop {
				let record = read_record(&mut server_read).await?;
				if replies.send(record).await.is_err() {
					return Ok::<(), io::Error>(());
				}
			}
		}
	};
	let from_client = async move {
		loop {
			let record = read_record(&mut client_read).await?;
			let Some(call) = Call::parse(&record) else {
				write_record(&mut server_write, &record).await?;
				continue;
			};
			if call.program == MOUNT_PROGRAM {
				if replies.send(mount(call, &adapter, &export)).await.is_err() {
					return Ok(());
				}
				continue;
			}
			let Some(call) = ReaddirCall::parse(call) else {
				write_record(&mut server_write, &record).await?;
				continue;
			};
			let adapter = adapter.clone();
			let replies = replies.clone();
			tokio::spawn(async move {
				let _ = replies.send(call.answer(&adapter).await).await;
			});
		}
		#[allow(unreachable_code)]
		Ok::<(), io::Error>(())
	};
	tokio::select! {
		result = writer => result,
		result = from_server => result,
		result = from_client => result,
		_ = stop.changed() => Ok(()),
	}
}

/// One RPC record: its fragments joined.
async fn read_record(stream: &mut (impl AsyncRead + Unpin)) -> io::Result<Vec<u8>> {
	let mut record = Vec::new();
	loop {
		let mark = stream.read_u32().await?;
		let len = (mark & 0x7fff_ffff) as usize;
		if record.len() + len > MAX_RECORD {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"RPC record too large",
			));
		}
		let start = record.len();
		record.resize(start + len, 0);
		stream.read_exact(&mut record[start..]).await?;
		if mark & 0x8000_0000 != 0 {
			return Ok(record);
		}
	}
}

async fn write_record(stream: &mut (impl AsyncWrite + Unpin), record: &[u8]) -> io::Result<()> {
	let mut framed = Vec::with_capacity(record.len() + 4);
	framed.extend_from_slice(&(0x8000_0000 | record.len() as u32).to_be_bytes());
	framed.extend_from_slice(record);
	stream.write_all(&framed).await
}

struct Reader<'a> {
	data: &'a [u8],
}

impl Reader<'_> {
	fn u32(&mut self) -> Option<u32> {
		let (head, rest) = self.data.split_first_chunk::<4>()?;
		self.data = rest;
		Some(u32::from_be_bytes(*head))
	}

	fn u64(&mut self) -> Option<u64> {
		let (head, rest) = self.data.split_first_chunk::<8>()?;
		self.data = rest;
		Some(u64::from_be_bytes(*head))
	}

	fn opaque(&mut self) -> Option<&[u8]> {
		let len = self.u32()? as usize;
		let padded = len.checked_add(3)? & !3;
		if self.data.len() < padded {
			return None;
		}
		let (body, rest) = self.data.split_at(padded);
		self.data = rest;
		Some(&body[..len])
	}
}

/// The header of an RPC call and a reader over its arguments.
struct Call<'a> {
	xid: u32,
	program: u32,
	version: u32,
	procedure: u32,
	args: Reader<'a>,
}

impl<'a> Call<'a> {
	/// `None` for anything but a well-formed RPC version 2 call.
	fn parse(record: &'a [u8]) -> Option<Self> {
		let mut reader = Reader { data: record };
		let xid = reader.u32()?;
		if (reader.u32()?, reader.u32()?) != (0, 2) {
			return None;
		}
		let (program, version, procedure) = (reader.u32()?, reader.u32()?, reader.u32()?);
		// Credentials and verifier: a flavor and an opaque body each.
		for _ in 0..2 {
			reader.u32()?;
			reader.opaque()?;
		}
		Some(Self {
			xid,
			program,
			version,
			procedure,
			args: reader,
		})
	}
}

/// The start of an accepted reply: the xid, REPLY, MSG_ACCEPTED, an empty
/// verifier, and `accept_stat`.
fn reply_header(xid: u32, accept_stat: u32) -> Vec<u8> {
	let mut out = Vec::new();
	for word in [xid, 1, 0, 0, 0, accept_stat] {
		out.extend_from_slice(&word.to_be_bytes());
	}
	out
}

/// The reply to a MOUNT program call.
fn mount(mut call: Call<'_>, adapter: &NfsAdapter, export: &Export) -> Vec<u8> {
	if call.version != MOUNT_VERSION {
		let mut out = reply_header(call.xid, PROG_MISMATCH);
		out.extend_from_slice(&MOUNT_VERSION.to_be_bytes());
		out.extend_from_slice(&MOUNT_VERSION.to_be_bytes());
		return out;
	}
	match call.procedure {
		MOUNTPROC_NULL | MOUNTPROC_UMNT | MOUNTPROC_UMNTALL => reply_header(call.xid, 0),
		// An empty mount list and an empty export list.
		MOUNTPROC_DUMP | MOUNTPROC_EXPORT => {
			let mut out = reply_header(call.xid, 0);
			out.extend_from_slice(&0u32.to_be_bytes());
			out
		}
		MOUNTPROC_MNT => {
			let Some(path) = call.args.opaque() else {
				return reply_header(call.xid, GARBAGE_ARGS);
			};
			let mut out = reply_header(call.xid, 0);
			if !export.claim(path) {
				debug!("NFS MOUNT refused");
				out.extend_from_slice(&MNT3ERR_ACCES.to_be_bytes());
				return out;
			}
			let root = adapter.id_to_fh(super::server::ROOT).data;
			out.extend_from_slice(&0u32.to_be_bytes());
			out.extend_from_slice(&(root.len() as u32).to_be_bytes());
			out.extend_from_slice(&root);
			out.resize(out.len() + (4 - root.len() % 4) % 4, 0);
			out.extend_from_slice(&1u32.to_be_bytes());
			out.extend_from_slice(&AUTH_UNIX.to_be_bytes());
			out
		}
		_ => reply_header(call.xid, PROC_UNAVAIL),
	}
}

/// A READDIR call, in the parts the reply needs.
struct ReaddirCall {
	xid: u32,
	dir: Vec<u8>,
	cookie: u64,
	count: u32,
}

impl ReaddirCall {
	/// `None` for anything but a well-formed NFSv3 READDIR, which nfsserve
	/// then answers or refuses itself.
	fn parse(call: Call<'_>) -> Option<Self> {
		if (call.program, call.version, call.procedure) != (NFS_PROGRAM, NFS_VERSION, READDIR) {
			return None;
		}
		let xid = call.xid;
		let mut reader = call.args;
		let dir = reader.opaque()?.to_vec();
		let cookie = reader.u64()?;
		reader.u64()?;
		let count = reader.u32()?;
		Some(Self {
			xid,
			dir,
			cookie,
			count,
		})
	}

	/// The whole reply record: as many entries after `cookie` as fit in
	/// `count` bytes, each with its file id as its cookie, as nfsserve's
	/// READDIRPLUS uses.
	async fn answer(self, adapter: &NfsAdapter) -> Vec<u8> {
		let mut out = reply_header(self.xid, 0);
		let page = self.page(adapter).await;
		let dir_attr = match page.dir_attr {
			Some(attr) => post_op_attr::attributes(attr),
			None => post_op_attr::Void,
		};
		let (entries, eof) = match page.result {
			Ok(page) => page,
			Err(status) => {
				out.extend_from_slice(&status.to_be_bytes());
				let _ = dir_attr.serialize(&mut out);
				return out;
			}
		};
		out.extend_from_slice(&0u32.to_be_bytes());
		let _ = dir_attr.serialize(&mut out);
		out.extend_from_slice(&[0; 8]);
		out.extend_from_slice(&entries);
		out.extend_from_slice(&0u32.to_be_bytes());
		out.extend_from_slice(&(eof as u32).to_be_bytes());
		out
	}

	async fn page(&self, adapter: &NfsAdapter) -> Page {
		let dir = match adapter.fh_to_id(&nfs_fh3 {
			data: self.dir.clone(),
		}) {
			Ok(dir) => dir,
			Err(status) => return Page::failed(status as u32, None),
		};
		let dir_attr = adapter.getattr(dir).await.ok();
		let (listing, start) = match adapter.page(dir, self.cookie).await {
			Ok(page) => page,
			Err(status) => return Page::failed(status as u32, dir_attr),
		};
		let budget = (self.count as usize).saturating_sub(READDIR_OVERHEAD);
		let mut entries = Vec::new();
		let mut written = 0;
		for entry in &listing[start..] {
			let name = super::server::nfs_name(&entry.name);
			let padded = (name.len() + 3) & !3;
			if entries.len() + 4 + 8 + 4 + padded + 8 > budget {
				break;
			}
			entries.extend_from_slice(&1u32.to_be_bytes());
			entries.extend_from_slice(&entry.id.to_be_bytes());
			entries.extend_from_slice(&(name.len() as u32).to_be_bytes());
			entries.extend_from_slice(&name);
			entries.resize(entries.len() + padded - name.len(), 0);
			entries.extend_from_slice(&entry.id.to_be_bytes());
			written += 1;
		}
		let remaining = listing.len() - start;
		if written == 0 && remaining > 0 {
			return Page::failed(NFS3ERR_TOOSMALL, dir_attr);
		}
		Page {
			dir_attr,
			result: Ok((entries, written == remaining)),
		}
	}
}

struct Page {
	dir_attr: Option<fattr3>,
	/// The encoded entries and whether they reach the end, or a status.
	result: Result<(Vec<u8>, bool), u32>,
}

impl Page {
	fn failed(status: u32, dir_attr: Option<fattr3>) -> Self {
		Self {
			dir_attr,
			result: Err(status),
		}
	}
}
