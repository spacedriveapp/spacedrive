use std::{
	io::{self},
	pin::Pin,
	sync::PoisonError,
	task::{Context, Poll},
};

use libp2p::{futures::AsyncWriteExt, PeerId, Stream};
use thiserror::Error;
use tokio::{
	io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt as TokioAsyncWriteExt, ReadBuf},
	sync::oneshot,
	time::{timeout, Duration},
};
use tokio_util::compat::Compat;

use crate::{
	spacetunnel::{Identity, IdentityErr, RemoteIdentity, REMOTE_IDENTITY_LEN},
	Manager,
};

pub const CHALLENGE_LENGTH: usize = 32;
const ONE_MINUTE: Duration = Duration::from_secs(60);

/// A unicast stream is a direct stream to a specific peer.
#[derive(Debug)]
#[allow(unused)] // TODO: Remove this lint override
pub struct UnicastStream {
	io: Compat<Stream>,
	me: Identity,
	remote: RemoteIdentity,
}

// TODO: Utils for sending msgpack and stuff over the stream. -> Have a max size of reading buffers so we are less susceptible to DoS attacks.

impl UnicastStream {
	pub(crate) async fn new_inbound(
		identity: Identity,
		mut io: Compat<Stream>,
	) -> Result<Self, UnicastStreamError> {
		// TODO: Finish this
		// let mut challenge = [0u8; CHALLENGE_LENGTH];
		// io.read_exact(&mut challenge).await.unwrap(); // TODO: Timeout
		// let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng); // 96-bits; unique per message
		// let ciphertext = cipher.encrypt(&nonce, b"plaintext message".as_ref())?;
		// let plaintext = cipher.decrypt(&nonce, ciphertext.as_ref())?;

		// TODO: THIS IS INSECURE!!!!!
		// We are just sending strings of the public key without any verification the other party holds the private key.
		let mut actual = [0; REMOTE_IDENTITY_LEN];
		match timeout(ONE_MINUTE, io.read_exact(&mut actual)).await {
			Ok(r) => r?,
			Err(_) => return Err(UnicastStreamError::Timeout),
		};
		let remote = RemoteIdentity::from_bytes(&actual)?;

		match timeout(
			ONE_MINUTE,
			io.write_all(&identity.to_remote_identity().get_bytes()),
		)
		.await
		{
			Ok(w) => w?,
			Err(_) => return Err(UnicastStreamError::Timeout),
		};

		// TODO: Do we have something to compare against? I don't think so this is fine.
		// if expected.get_bytes() != actual {
		// 	panic!("Mismatch in remote identity!");
		// }

		Ok(Self {
			io,
			me: identity,
			remote,
		})
	}

	pub(crate) async fn new_outbound(
		identity: Identity,
		mut io: Compat<Stream>,
	) -> Result<Self, UnicastStreamError> {
		// TODO: Use SPAKE not some handrolled insecure mess
		// let challenge = rand::thread_rng().gen::<[u8; CHALLENGE_LENGTH]>();
		// self.0.write_all(&challenge).await?;

		// TODO: THIS IS INSECURE!!!!!
		// We are just sending strings of the public key without any verification the other party holds the private key.
		match timeout(
			ONE_MINUTE,
			io.write_all(&identity.to_remote_identity().get_bytes()),
		)
		.await
		{
			Ok(w) => w?,
			Err(_) => return Err(UnicastStreamError::Timeout),
		};

		let mut actual = [0; REMOTE_IDENTITY_LEN];
		match timeout(ONE_MINUTE, io.read_exact(&mut actual)).await {
			Ok(r) => r?,
			Err(_) => return Err(UnicastStreamError::Timeout),
		};
		let remote = RemoteIdentity::from_bytes(&actual)?;

		// TODO: Do we have something to compare against? I don't think so this is fine.
		// if expected.get_bytes() != actual {
		// 	panic!("Mismatch in remote identity!");
		// }

		Ok(Self {
			io,
			me: identity,
			remote,
		})
	}

	#[must_use]
	pub fn remote_identity(&self) -> RemoteIdentity {
		self.remote
	}

	pub async fn close(self) -> Result<(), io::Error> {
		self.io.into_inner().close().await
	}
}

impl AsyncRead for UnicastStream {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().io).poll_read(cx, buf)
	}
}

impl AsyncWrite for UnicastStream {
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		Pin::new(&mut self.get_mut().io).poll_write(cx, buf)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().io).poll_flush(cx)
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().io).poll_shutdown(cx)
	}
}

#[derive(Debug, Error)]
pub enum UnicastStreamError {
	#[error("io error: {0}")]
	IoError(#[from] io::Error),
	#[error("identity error: {0}")]
	InvalidError(#[from] IdentityErr),
	// TODO: Technically this error is from the manager
	#[error("peer id not found")]
	PeerIdNotFound,
	#[error("error manager shutdown")]
	ErrManagerShutdown(#[from] oneshot::error::RecvError),
	#[error("error getting peer id for '{0}'")]
	ErrPeerIdNotFound(RemoteIdentity),
	#[error("timeout")]
	Timeout,
}

#[derive(Debug)]
pub struct UnicastStreamBuilder {
	identity: Identity,
	io: Compat<Stream>,
}

impl UnicastStreamBuilder {
	pub(crate) fn new(identity: Identity, io: Compat<Stream>) -> Self {
		Self { identity, io }
	}

	pub(crate) async fn build(
		self,
		manager: &Manager,
		peer_id: PeerId,
	) -> Result<UnicastStream, UnicastStreamError> {
		let stream = UnicastStream::new_outbound(self.identity, self.io).await?;

		manager
			.state
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.connected
			.insert(peer_id, stream.remote_identity());

		Ok(stream)
	}
}
