use crate::{
	primitives::{EncryptedBlock, EncryptedBlockRef, StreamNonce},
	Error,
};

use std::future::Future;

use aead::{stream::DecryptorLE31, Aead, KeyInit};
use chacha20poly1305::XChaCha20Poly1305;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter};

use super::secret_key::SecretKey;

pub trait OneShotDecryption {
	fn decrypt(&self, cipher_text: EncryptedBlockRef<'_>) -> Result<Vec<u8>, Error>;
	fn decrypt_owned(&self, cipher_text: &EncryptedBlock) -> Result<Vec<u8>, Error>;
}

pub trait StreamDecryption {
	fn decrypt(
		&self,
		nonce: &StreamNonce,
		reader: impl AsyncRead + Unpin + Send,
		writer: impl AsyncWrite + Unpin + Send,
	) -> impl Future<Output = Result<(), Error>> + Send;
}

impl OneShotDecryption for SecretKey {
	fn decrypt(
		&self,
		EncryptedBlockRef { nonce, cipher_text }: EncryptedBlockRef<'_>,
	) -> Result<Vec<u8>, Error> {
		XChaCha20Poly1305::new(&self.0)
			.decrypt(nonce, cipher_text)
			.map_err(|aead::Error| Error::Decrypt)
	}

	fn decrypt_owned(
		&self,
		EncryptedBlock { nonce, cipher_text }: &EncryptedBlock,
	) -> Result<Vec<u8>, Error> {
		XChaCha20Poly1305::new(&self.0)
			.decrypt(nonce, cipher_text.as_slice())
			.map_err(|aead::Error| Error::Decrypt)
	}
}

impl StreamDecryption for SecretKey {
	async fn decrypt(
		&self,
		nonce: &StreamNonce,
		reader: impl AsyncRead + Unpin + Send,
		writer: impl AsyncWrite + Unpin + Send,
	) -> Result<(), Error> {
		let mut reader = BufReader::with_capacity(EncryptedBlock::CIPHER_TEXT_SIZE, reader);
		let mut writer = BufWriter::with_capacity(EncryptedBlock::PLAIN_TEXT_SIZE * 5, writer);

		let mut buf = Vec::with_capacity(EncryptedBlock::CIPHER_TEXT_SIZE);

		let mut decryptor = DecryptorLE31::from_aead(XChaCha20Poly1305::new(&self.0), nonce);

		loop {
			match reader.fill_buf().await {
				Ok([]) => {
					// Jobs done
					break;
				}

				Ok(bytes) => {
					let total_bytes = bytes.len();

					buf.clear();
					buf.extend_from_slice(bytes);

					reader.consume(total_bytes);

					if total_bytes == EncryptedBlock::CIPHER_TEXT_SIZE {
						decryptor
							.decrypt_next_in_place(b"", &mut buf)
							.map_err(|aead::Error| Error::Decrypt)?;

						writer.write_all(&buf).await.map_err(|e| Error::DecryptIo {
							context: "Writing decrypted block to writer",
							source: e,
						})?;
					} else {
						decryptor
							.decrypt_last_in_place(b"", &mut buf)
							.map_err(|aead::Error| Error::Decrypt)?;

						writer.write_all(&buf).await.map_err(|e| Error::DecryptIo {
							context: "Writing last decrypted block to writer",
							source: e,
						})?;
						break;
					}
				}

				Err(e) => {
					return Err(Error::DecryptIo {
						context: "Reading a block from the reader",
						source: e,
					});
				}
			}
		}

		writer.flush().await.map_err(|e| Error::DecryptIo {
			context: "Flushing writer",
			source: e,
		})?;

		Ok(())
	}
}
