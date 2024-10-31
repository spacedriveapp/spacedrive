//! A system for creating encrypted tunnels between peers over untrusted connections.

use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use sd_p2p_proto::{decode, encode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use thiserror::Error;

use sd_p2p::{Identity, IdentityErr, RemoteIdentity, UnicastStream};

#[derive(Debug, Error)]
pub enum TunnelError {
	#[error("Error writing discriminator.")]
	DiscriminatorWriteError,
	#[error("Error reading discriminator. Is this stream actually a tunnel?")]
	DiscriminatorReadError,
	#[error("Invalid discriminator. Is this stream actually a tunnel?")]
	InvalidDiscriminator,
	#[error("Error sending library id: {0:?}")]
	ErrorSendingLibraryId(io::Error),
	#[error("Error receiving library identity: {0:?}")]
	ErrorReceivingLibraryIdentity(decode::Error),
	#[error("Error decoding library identity: {0:?}")]
	ErrorDecodingLibraryIdentity(IdentityErr),
}

/// An encrypted tunnel between two libraries.
///
/// This sits on top of the existing node to node encryption provided by Quic.
///
/// It's primarily designed to avoid an attack where traffic flows:
///     node <-> attacker node <-> node
/// The attackers node can't break TLS but if they get in the middle they can present their own node identity to each side and then intercept library related traffic.
/// To avoid that we use this tunnel to encrypt all library related traffic so it can only be decoded by another instance of the same library.
#[derive(Debug)]
pub struct Tunnel {
	stream: UnicastStream,
	library_remote_id: RemoteIdentity,
}

impl Tunnel {
	/// Create a new tunnel.
	///
	/// This should be used by the node that initiated the request which this tunnel is used for.
	pub async fn initiator(
		mut stream: UnicastStream,
		library_identity: &Identity,
	) -> Result<Self, TunnelError> {
		stream
			.write_all(b"T")
			.await
			.map_err(|_| TunnelError::DiscriminatorWriteError)?;

		let mut buf = vec![];
		encode::buf(&mut buf, &library_identity.to_remote_identity().get_bytes());
		stream
			.write_all(&buf)
			.await
			.map_err(TunnelError::ErrorSendingLibraryId)?;

		// TODO: Do encryption things

		Ok(Self {
			stream,
			library_remote_id: library_identity.to_remote_identity(),
		})
	}

	/// Create a new tunnel.
	///
	/// This should be used by the node that responded to the request which this tunnel is used for.
	pub async fn responder(mut stream: UnicastStream) -> Result<Self, TunnelError> {
		let discriminator = stream
			.read_u8()
			.await
			.map_err(|_| TunnelError::DiscriminatorReadError)?;
		if discriminator != b'T' {
			return Err(TunnelError::InvalidDiscriminator);
		}

		// TODO: Blindly decoding this from the stream is not secure. We need a cryptographic handshake here to prove the peer on the other ends is holding the private key.
		let library_remote_id = decode::buf(&mut stream)
			.await
			.map_err(TunnelError::ErrorReceivingLibraryIdentity)?;

		let library_remote_id = RemoteIdentity::from_bytes(&library_remote_id)
			.map_err(TunnelError::ErrorDecodingLibraryIdentity)?;

		// TODO: Do encryption things

		Ok(Self {
			library_remote_id,
			stream,
		})
	}

	/// Get the `RemoteIdentity` of the peer on the other end of the tunnel.
	pub fn node_remote_identity(&self) -> RemoteIdentity {
		self.stream.remote_identity()
	}

	/// Get the `RemoteIdentity` of the library instance on the other end of the tunnel.
	pub fn library_remote_identity(&self) -> RemoteIdentity {
		self.library_remote_id
	}
}

impl AsyncRead for Tunnel {
	fn poll_read(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &mut ReadBuf<'_>,
	) -> Poll<io::Result<()>> {
		// TODO: Do decryption

		Pin::new(&mut self.get_mut().stream).poll_read(cx, buf)
	}
}

impl AsyncWrite for Tunnel {
	fn poll_write(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
		buf: &[u8],
	) -> Poll<io::Result<usize>> {
		// TODO: Do encryption

		Pin::new(&mut self.get_mut().stream).poll_write(cx, buf)
	}

	fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().stream).poll_flush(cx)
	}

	fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
		Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
	}
}
