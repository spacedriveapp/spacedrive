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

use super::server::NfsAdapter;
use nfsserve::nfs::{fattr3, nfs_fh3, post_op_attr};
use nfsserve::vfs::NFSFileSystem;
use nfsserve::xdr::XDR;
use std::io;
use std::net::SocketAddr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tracing::debug;

const NFS_PROGRAM: u32 = 100003;
const NFS_VERSION: u32 = 3;
const READDIR: u32 = 16;
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
	mut stop: watch::Receiver<bool>,
) {
	loop {
		tokio::select! {
			accepted = listener.accept() => {
				let Ok((client, _)) = accepted else {
					continue;
				};
				let adapter = adapter.clone();
				let stop = stop.clone();
				tokio::spawn(async move {
					if let Err(err) = connection(client, backend, adapter, stop).await {
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
			let Some(call) = ReaddirCall::parse(&record) else {
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
	fn parse(record: &[u8]) -> Option<Self> {
		let mut reader = Reader { data: record };
		let xid = reader.u32()?;
		let call = (
			reader.u32()?,
			reader.u32()?,
			reader.u32()?,
			reader.u32()?,
			reader.u32()?,
		);
		if call != (0, 2, NFS_PROGRAM, NFS_VERSION, READDIR) {
			return None;
		}
		// Credentials and verifier: a flavor and an opaque body each.
		for _ in 0..2 {
			reader.u32()?;
			reader.opaque()?;
		}
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
		let mut out = Vec::new();
		for word in [self.xid, 1, 0, 0, 0, 0] {
			out.extend_from_slice(&word.to_be_bytes());
		}
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
