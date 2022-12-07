//! Connection: TODO

use std::{fmt, marker::PhantomData, net::SocketAddr};

use serde::Serialize;
use thiserror::Error;

use crate::{PeerId, SendError, Stream, TransportConnection};

pub struct Connection<TPayload, T> {
	conn: T,
	phantom: PhantomData<TPayload>,
}

impl<TPayload, T> Connection<TPayload, T>
where
	TPayload: Serialize,
	T: TransportConnection,
{
	pub(crate) fn new(conn: T) -> Self {
		Self {
			conn,
			phantom: PhantomData,
		}
	}

	/// TODO
	pub fn peer_id(&self) -> Result<PeerId, String> {
		self.conn.peer_id()
	}

	/// TODO
	pub fn remote_addr(&self) -> SocketAddr {
		self.conn.remote_addr()
	}

	/// TODO
	pub async fn stream(
		&self,
		payload: TPayload,
	) -> Result<Stream<T::Stream>, StreamConnectError<T>> {
		let mut stream = Stream::new(
			self.conn.accept_stream(
				self.conn
					.stream()
					.await
					.map_err(StreamConnectError::TransportError)?,
			),
		);
		stream.send(payload).await?;
		Ok(stream)
	}

	/// TODO
	pub fn close(self) {
		self.conn.close();
	}
}

#[derive(Error)]
pub enum StreamConnectError<T: TransportConnection> {
	#[error("stream connect transport error: {0}")]
	TransportError(T::Error),
	#[error("stream connect establishment payload send error: {0}")]
	SendError(#[from] SendError),
}

// Using derive for this impl will force the bound `T: Debug` which as shown here is unnecessary
impl<T: TransportConnection> fmt::Debug for StreamConnectError<T> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::TransportError(err) => write!(f, "TransportError({:?})", err),
			Self::SendError(err) => write!(f, "SendError({:?})", err),
		}
	}
}
