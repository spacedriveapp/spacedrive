//! Stream: TODO

use futures_util::AsyncWriteExt;
use serde::Serialize;
use thiserror::Error;

use crate::ConnectionStream;

/// TODO
pub struct Stream<TStream> {
	stream: TStream,
	// TODO: Hold tx, rx
	// TODO: Hold stream controller
}

impl<TStream: ConnectionStream> Stream<TStream> {
	pub fn new(stream: TStream) -> Self {
		Self { stream }
	}

	// fn peer_id(&self) -> PeerId {}
	// fn remote_addr(&self) -> SocketAddr {}

	pub async fn send<T: Serialize>(&mut self, t: T) -> Result<(), SendError> {
		let bytes = rmp_serde::to_vec_named(&t)?;
		self.stream.write(&bytes[..]).await?;
		Ok(())
	}

	fn close(&self) {
		todo!();
	}
}

// impl AsyncWrite

// impl AsyncRead

// TODO: Helpers for AsyncRead/AsyncWrite + MsgPack

#[derive(Error, Debug)]
pub enum SendError {
	#[error("msgpack encoding error sending message: {0}")]
	MsgpackError(#[from] rmp_serde::encode::Error),
	#[error("io error sending message: {0}")]
	IoError(#[from] std::io::Error),
}
