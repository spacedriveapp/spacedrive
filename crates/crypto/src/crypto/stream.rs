use std::io::Cursor;

use crate::{
	primitives::{AEAD_TAG_LEN, BLOCK_LEN},
	types::{Algorithm, Key, Nonce},
	Error, Protected, Result,
};
use aead::{
	stream::{DecryptorLE31, EncryptorLE31},
	Payload,
};
use aes_gcm::Aes256Gcm;
use chacha20poly1305::XChaCha20Poly1305;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::exhaustive_read;

macro_rules! impl_stream {
	(
	$name:ident, // "Decryptor", "Encryptor"
	$error:expr,
	$next_fn:ident, // "encrypt_next"
	$last_fn:ident, // "encrypt_last"
	$stream_primitive:ident, // "DecryptorLE31"
	$streams_fn:ident, // "encrypt_streams"
	$bytes_fn:ident, // "encrypt_bytes"
	$bytes_return:ty,
	$size:expr,
	$($algorithm:tt),*
) => {
		pub enum $name {
			$(
				$algorithm(Box<$stream_primitive<$algorithm>>),
			)*
		}

		impl $name {
			/// This should be used to initialize a stream object.
			///
			/// The desired master key, nonce and algorithm should be provided.
			#[allow(clippy::needless_pass_by_value)]
			pub fn new(key: Key, nonce: Nonce, algorithm: Algorithm) -> Result<Self> {
				if nonce.len() != algorithm.nonce_len() {
					return Err(Error::NonceLengthMismatch);
				}

				let s = match algorithm {
					$(
						Algorithm::$algorithm => Self::$algorithm(Box::new($stream_primitive::new(&key.into(), &nonce.into()))),
					)*
				};

				Ok(s)
			}

			fn $next_fn<'msg, 'aad>(
				&mut self,
				payload: impl Into<Payload<'msg, 'aad>>,
			) -> Result<Vec<u8>> {
				match self {
					$(
						Self::$algorithm(s) => s.$next_fn(payload),
					)*
				}
				.map_err(|_| $error)
			}

			fn $last_fn<'msg, 'aad>(self, payload: impl Into<Payload<'msg, 'aad>>) -> Result<Vec<u8>> {
				match self {
					$(
						Self::$algorithm(s) => s.$last_fn(payload),
					)*
				}
				.map_err(|_| $error)
			}

			/// This function should be used for large amounts of data.
			///
			/// The streaming implementation reads blocks of data in `BLOCK_LEN`, encrypts/decrypts, and writes to the writer.
			///
			/// It requires a reader, a writer, and any relevant AAD.
			///
			/// The AAD will be authenticated with every block of data.
			pub async fn $streams_fn<R, W>(
				mut self,
				mut reader: R,
				mut writer: W,
				aad: &[u8],
			) -> Result<()>
			where
				R: AsyncReadExt + Unpin + Send,
				W: AsyncWriteExt + Unpin + Send,
			{
				let mut buffer = vec![0u8; $size].into_boxed_slice();

				loop {
					let count = exhaustive_read(&mut reader, &mut buffer).await?;

					let payload = Payload {
						aad,
						msg: &buffer[..count],
					};

					if count == $size {
						let d = self.$next_fn(payload)?;
						writer.write_all(&d).await?;
					} else {
						let d = self.$last_fn(payload)?;
						writer.write_all(&d).await?;
						break;
					}
				}

				writer.flush().await?;

				Ok(())
			}

			/// This should ideally only be used for small amounts of data.
			///
			/// It is just a thin wrapper around the associated `encrypt/decrypt_streams` function.
			#[allow(unused_mut)]
			pub async fn $bytes_fn(
				key: Key,
				nonce: Nonce,
				algorithm: Algorithm,
				bytes: &[u8],
				aad: &[u8],
			) -> Result<$bytes_return> {
				let mut writer = Cursor::new(Vec::new());
				let s = Self::new(key, nonce, algorithm)?;

				s
					.$streams_fn(bytes, &mut writer, aad)
					.await
					.map_or_else(Err, |()| Ok(writer.into_inner().into()))
			}

		}
	};
}

impl_stream!(
	Encryptor,
	Error::Encrypt,
	encrypt_next,
	encrypt_last,
	EncryptorLE31,
	encrypt_streams,
	encrypt_bytes,
	Vec<u8>,
	BLOCK_LEN,
	XChaCha20Poly1305,
	Aes256Gcm
);

impl_stream!(
	Decryptor,
	Error::Decrypt,
	decrypt_next,
	decrypt_last,
	DecryptorLE31,
	decrypt_streams,
	decrypt_bytes,
	Protected<Vec<u8>>,
	(BLOCK_LEN + AEAD_TAG_LEN),
	XChaCha20Poly1305,
	Aes256Gcm
);
