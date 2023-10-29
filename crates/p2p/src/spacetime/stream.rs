use std::{
	io::{self, ErrorKind},
	pin::Pin,
	sync::PoisonError,
	task::{Context, Poll},
};

use libp2p::{futures::AsyncWriteExt, PeerId, Stream};
use tokio::io::{
	AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt as TokioAsyncWriteExt, ReadBuf,
};
use tokio_util::compat::Compat;

use crate::{
	spacetunnel::{Identity, RemoteIdentity, REMOTE_IDENTITY_LEN},
	Manager,
};

pub const BROADCAST_DISCRIMINATOR: u8 = 0;
pub const UNICAST_DISCRIMINATOR: u8 = 1;

pub const CHALLENGE_LENGTH: usize = 32;

/// A broadcast is a message sent to many peers in the network.
/// Due to this it is not possible to respond to a broadcast.
#[derive(Debug)]
pub struct BroadcastStream(Option<Compat<Stream>>);

impl BroadcastStream {
	#[allow(unused)]
	pub(crate) fn new(stream: Compat<Stream>) -> Self {
		Self(Some(stream))
	}

	async fn close_inner(mut io: Compat<Stream>) -> Result<(), io::Error> {
		io.write_all(&[b'D']).await?;
		io.flush().await?;

		match io.into_inner().close().await {
			Ok(_) => Ok(()),
			Err(err) if err.kind() == ErrorKind::ConnectionReset => Ok(()), // The other end shut the connection before us
			Err(err) => Err(err),
		}
	}
}

impl AsyncRead for BroadcastStream {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().0.as_mut().expect("'BroadcastStream' can only be 'None' if this method is called after 'Drop' which ain't happening!")).poll_read(cx, buf)
	}
}

impl Drop for BroadcastStream {
	fn drop(&mut self) {
		// This may be `None` if the user manually called `Self::close`
		if let Some(stream) = self.0.take() {
			tokio::spawn(async move {
				Self::close_inner(stream).await.unwrap();
			});
		}
	}
}

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
	pub(crate) async fn new_inbound(identity: Identity, mut io: Compat<Stream>) -> Self {
		// TODO: Finish this
		// let mut challenge = [0u8; CHALLENGE_LENGTH];
		// io.read_exact(&mut challenge).await.unwrap(); // TODO: Timeout
		// let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng); // 96-bits; unique per message
		// let ciphertext = cipher.encrypt(&nonce, b"plaintext message".as_ref())?;
		// let plaintext = cipher.decrypt(&nonce, ciphertext.as_ref())?;

		// TODO: THIS IS INSECURE!!!!!
		// We are just sending strings of the public key without any verification the other party holds the private key.
		let mut actual = [0; REMOTE_IDENTITY_LEN];
		io.read_exact(&mut actual).await.unwrap(); // TODO: Error handling + timeout
		let remote = RemoteIdentity::from_bytes(&actual).unwrap(); // TODO: Error handling

		io.write_all(&identity.to_remote_identity().get_bytes())
			.await
			.unwrap(); // TODO: Error handling + timeout

		// TODO: Do we have something to compare against? I don't think so this is fine.
		// if expected.get_bytes() != actual {
		// 	panic!("Mismatch in remote identity!");
		// }

		Self {
			io,
			me: identity,
			remote,
		}
	}

	pub(crate) async fn new_outbound(identity: Identity, mut io: Compat<Stream>) -> Self {
		// TODO: Use SPAKE not some handrolled insecure mess
		// let challenge = rand::thread_rng().gen::<[u8; CHALLENGE_LENGTH]>();
		// self.0.write_all(&challenge).await?;

		// TODO: THIS IS INSECURE!!!!!
		// We are just sending strings of the public key without any verification the other party holds the private key.
		io.write_all(&identity.to_remote_identity().get_bytes())
			.await
			.unwrap(); // TODO: Timeout

		let mut actual = [0; REMOTE_IDENTITY_LEN];
		io.read_exact(&mut actual).await.unwrap(); // TODO: Timeout
		let remote = RemoteIdentity::from_bytes(&actual).unwrap(); // TODO: Error handling

		// TODO: Do we have something to compare against? I don't think so this is fine.
		// if expected.get_bytes() != actual {
		// 	panic!("Mismatch in remote identity!");
		// }

		Self {
			io,
			me: identity,
			remote,
		}
	}

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

#[derive(Debug)]
pub struct UnicastStreamBuilder {
	identity: Identity,
	io: Compat<Stream>,
}

impl UnicastStreamBuilder {
	pub(crate) fn new(identity: Identity, io: Compat<Stream>) -> Self {
		Self { identity, io }
	}

	pub(crate) async fn build(mut self, manager: &Manager, peer_id: PeerId) -> UnicastStream {
		// TODO: Timeout if the peer doesn't accept the byte quick enough
		self.io.write_all(&[UNICAST_DISCRIMINATOR]).await.unwrap();

		let stream = UnicastStream::new_outbound(self.identity, self.io).await;

		manager
			.state
			.write()
			.unwrap_or_else(PoisonError::into_inner)
			.connected
			.insert(peer_id, stream.remote_identity());

		stream
	}
}
