//! A system for creating encrypted tunnels between peers over untrusted connections.

use std::{
	io,
	pin::Pin,
	task::{Context, Poll},
};

use sd_p2p_proto::{decode, encode};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use thiserror::Error;

use sd_p2p::{Identity, RemoteIdentity, UnicastStream};
use uuid::Uuid;

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
	#[error("Error receiving library id: {0:?}")]
	ErrorReceivingLibraryId(decode::Error),
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
	library_id: Uuid,
}

impl Tunnel {
	/// Create a new tunnel.
	///
	/// This should be used by the node that initiated the request which this tunnel is used for.
	pub async fn initiator(
		mut stream: UnicastStream,
		library_id: &Uuid,
		library_identity: &Identity,
	) -> Result<Self, TunnelError> {
		stream
			.write_all(&[b'T'])
			.await
			.map_err(|_| TunnelError::DiscriminatorWriteError)?;

		let mut buf = vec![];
		encode::uuid(&mut buf, library_id);
		stream
			.write_all(&buf)
			.await
			.map_err(TunnelError::ErrorSendingLibraryId)?;

		// TODO: Do encryption tings

		Ok(Self {
			stream,
			library_id: *library_id,
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

		let library_id = decode::uuid(&mut stream)
			.await
			.map_err(TunnelError::ErrorReceivingLibraryId)?;

		// TODO: Do encryption tings

		Ok(Self {
			// TODO: This is wrong but it's fine for now cause we don't use it.
			// TODO: Will fix this in a follow up PR when I add encryption
			library_remote_id: stream.remote_identity(),
			stream,
			library_id,
		})
	}

	/// The the ID of the library being tunneled.
	pub fn library_id(&self) -> Uuid {
		self.library_id
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
