use quinn::{RecvStream, SendStream};
use serde::{de::DeserializeOwned, Serialize};
use thiserror::Error;

/// MAX_MESSAGE_SIZE is the maximum size of a single message.
pub const MAX_MESSAGE_SIZE: usize = 64 * 1024;

#[derive(Error, Debug)]
pub enum UtilError {
	#[error("error reading from stream as it was closed")]
	StreamClosed,
	#[error("error writing message")]
	WriteError(#[from] quinn::WriteError),
	#[error("error reading data")]
	ReadError(#[from] quinn::ReadError),
	#[error("error decoding message")]
	DecodeError(#[from] rmp_serde::decode::Error),
	#[error("error encoding message")]
	EncodeError(#[from] rmp_serde::encode::Error),
}

// write_value is a helper to write a Serde struct to a [quin::SendStream].
pub async fn write_value<T>(tx: &mut SendStream, value: &T) -> Result<(), UtilError>
where
	T: Serialize + Unpin + ?Sized,
{
	let data = rmp_serde::encode::to_vec_named(value)?;
	// rmp_serde doesn't support `AsyncWrite` so we have to allocate buffer here.
	tx.write_all(&data).await?;
	Ok(())
}

// read_value is a helper to read a Serde struct from a [quin::RecvStream].
pub async fn read_value<T>(rx: &mut RecvStream) -> Result<T, UtilError>
where
	T: DeserializeOwned + ?Sized,
{
	let data = rx
		.read_chunk(MAX_MESSAGE_SIZE, true)
		.await?
		.ok_or(UtilError::StreamClosed)?;
	Ok(rmp_serde::decode::from_read(&data.bytes[..])?)
}
