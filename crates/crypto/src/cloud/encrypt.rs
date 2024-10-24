use crate::{
	primitives::{EncryptedBlock, OneShotNonce, StreamNonce},
	Error,
};

use aead::{stream::EncryptorLE31, Aead, KeyInit};
use async_stream::stream;
use chacha20poly1305::{Tag, XChaCha20Poly1305, XNonce};
use futures::Stream;
use rand::CryptoRng;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

use super::secret_key::SecretKey;

pub trait OneShotEncryption {
	fn encrypt(&self, plaintext: &[u8], rng: &mut impl CryptoRng) -> Result<EncryptedBlock, Error>;

	fn cipher_text_size(&self, plain_text_size: usize) -> usize {
		size_of::<OneShotNonce>() + plain_text_size + size_of::<Tag>()
	}
}

pub trait StreamEncryption {
	fn encrypt(
		&self,
		reader: impl AsyncRead + Unpin + Send,
		rng: &mut (impl CryptoRng + Send),
	) -> (
		StreamNonce,
		impl Stream<Item = Result<Vec<u8>, Error>> + Send,
	);

	fn cipher_text_size(&self, plain_text_size: usize) -> usize {
		size_of::<StreamNonce>()
			+ (plain_text_size / EncryptedBlock::PLAIN_TEXT_SIZE * EncryptedBlock::CIPHER_TEXT_SIZE)
			+ plain_text_size % EncryptedBlock::PLAIN_TEXT_SIZE
			+ size_of::<Tag>()
	}
}

impl OneShotEncryption for SecretKey {
	fn encrypt(&self, plaintext: &[u8], rng: &mut impl CryptoRng) -> Result<EncryptedBlock, Error> {
		if plaintext.len() > EncryptedBlock::PLAIN_TEXT_SIZE {
			return Err(Error::BlockTooBig(plaintext.len()));
		}

		let cipher = XChaCha20Poly1305::new(&self.0);
		let mut nonce = XNonce::default();
		rng.fill_bytes(&mut nonce);

		Ok(EncryptedBlock {
			nonce,
			cipher_text: cipher
				.encrypt(&nonce, plaintext)
				.map_err(|aead::Error| Error::Encrypt)?,
		})
	}
}

impl StreamEncryption for SecretKey {
	fn encrypt(
		&self,
		reader: impl AsyncRead + Unpin + Send,
		rng: &mut (impl CryptoRng + Send),
	) -> (
		StreamNonce,
		impl Stream<Item = Result<Vec<u8>, Error>> + Send,
	) {
		let mut nonce = StreamNonce::default();
		rng.fill_bytes(&mut nonce);

		(
			nonce,
			stream! {
				let mut reader = BufReader::with_capacity(EncryptedBlock::PLAIN_TEXT_SIZE, reader);
				let mut encryptor = EncryptorLE31::from_aead(XChaCha20Poly1305::new(&self.0), &nonce);

				loop {
					match reader.fill_buf().await {
						Ok([]) => {
							// Jobs done
							break;
						}

						Ok(bytes) => {
							let total_bytes = bytes.len();
							if bytes.len() == EncryptedBlock::PLAIN_TEXT_SIZE {
								let cipher_text = encryptor.encrypt_next(bytes).map_err(|aead::Error| Error::Encrypt)?;
								assert_eq!(cipher_text.len(), EncryptedBlock::CIPHER_TEXT_SIZE);
								yield Ok(cipher_text);
								reader.consume(total_bytes);
							} else {
								yield encryptor.encrypt_last(bytes).map_err(|aead::Error| Error::Encrypt);
								break;
							}
						}

						Err(e) => {
							yield Err(Error::EncryptIo {
								context: "Reading a block from the reader",
								source: e,
							});
							break;
						}
					}
				}
			},
		)
	}
}
