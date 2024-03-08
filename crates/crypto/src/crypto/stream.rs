use std::io::{Cursor, Read, Write};

use crate::{
	primitives::{AEAD_TAG_LEN, BLOCK_LEN},
	types::{Aad, Algorithm, EncryptedKey, Key, Nonce},
	utils::ToArray,
	Error, Protected, Result,
};
use aead::{
	stream::{DecryptorLE31, EncryptorLE31},
	Payload,
};
use aes_gcm_siv::Aes256GcmSiv;
use chacha20poly1305::XChaCha20Poly1305;

#[cfg(feature = "tokio")]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

macro_rules! impl_stream {
	(
	$name:ident, // "Decryptor", "Encryptor"
	$error:expr,
	$next_fn:ident, // "encrypt_next"
	$last_fn:ident, // "encrypt_last"
	$last_in_place_fn:ident,
	$stream_primitive:ident, // "DecryptorLE31"
	$streams_fn:ident, // "encrypt_streams"
	$streams_fn_async:ident, // "encrypt_streams_async"
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
			///
			/// This function ensures that both the nonce and key are *valid*.
			/// For more information, view `Key::validate()` and `Nonce::validate()`
			pub fn new(key: &Key, nonce: &Nonce, algorithm: Algorithm) -> Result<Self> {
				nonce.validate(algorithm)?;
				key.validate()?;

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

			fn $last_in_place_fn(self, aad: Aad, buf: &mut dyn aead::Buffer) -> Result<()> {
				match self {
					$(
						Self::$algorithm(s) => s.$last_in_place_fn(aad.inner(), buf),
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
			pub fn $streams_fn<R, W>(
				mut self,
				mut reader: R,
				mut writer: W,
				aad: Aad,
			) -> Result<()>
			where
				R: Read,
				W: Write,
			{
				let mut buffer = vec![0u8; $size].into_boxed_slice();

				loop {
					let count = reader.read(&mut buffer)?;

					let payload = Payload {
						aad: aad.inner(),
						msg: &buffer[..count],
					};

					if count == $size {
						let data = self.$next_fn(payload)?;
						writer.write_all(&data)?;
					} else {
						let data = self.$last_fn(payload)?;
						writer.write_all(&data)?;
						break;
					}
				}

				writer.flush()?;

				Ok(())
			}

			/// This function should be used for large amounts of data.
			///
			/// The streaming implementation reads blocks of data in `BLOCK_LEN`, encrypts/decrypts, and writes to the writer.
			///
			/// It requires a reader, a writer, and any relevant AAD.
			///
			/// The AAD will be authenticated with every block of data.
			#[cfg(feature = "tokio")]
			pub async fn $streams_fn_async<R, W>(
				mut self,
				mut reader: R,
				mut writer: W,
				aad: Aad,
			) -> Result<()>
			where
				R: AsyncReadExt + Unpin + Send,
				W: AsyncWriteExt + Unpin + Send,
			{
				let mut buffer = vec![0u8; $size].into_boxed_slice();

				loop {
					let count = reader.read(&mut buffer).await?;

					// TODO(brxken128): block on `next_fn` and `last_fn` exclusively

					let payload = Payload {
						aad: aad.inner(),
						msg: &buffer[..count],
					};

					if count == $size {
						let data = self.$next_fn(payload)?;
						writer.write_all(&data).await?;
					} else {
						let data = self.$last_fn(payload)?;
						writer.write_all(&data).await?;
						break;
					}
				}

				writer.flush().await?;

				Ok(())
			}

			/// This should ideally only be used for small amounts of data.
			///
			/// It is just a thin wrapper around the associated `encrypt/decrypt_streams` function.
			pub fn $bytes_fn(
				key: &Key,
				nonce: &Nonce,
				algorithm: Algorithm,
				bytes: &[u8],
				aad: Aad,
			) -> Result<$bytes_return> {
				let mut writer = Cursor::new(Vec::new());
				let s = Self::new(key, nonce, algorithm)?;

				s
					.$streams_fn(bytes, &mut writer, aad)
					.map(|()| writer.into_inner().into())
			}
		}
	};
}

impl Encryptor {
	pub fn encrypt_key(
		key: &Key,
		nonce: &Nonce,
		algorithm: Algorithm,
		key_to_encrypt: &Key,
		aad: Aad,
	) -> Result<EncryptedKey> {
		Self::encrypt_tiny(key, nonce, algorithm, key_to_encrypt.expose(), aad)
			.map(|b| Ok(EncryptedKey::new(b.to_array()?, *nonce)))
			.map_err(|_| Error::Encrypt)?
	}

	/// This is only for encrypting inputs < `BLOCK_LEN`. For anything larger,
	/// see [`Encryptor::encrypt_bytes`] or [`Encryptor::encrypt_streams`].
	///
	/// It uses `encrypt_last_in_place` under the hood due to the input always being less than `BLOCK_LEN`.
	///
	/// It's faster than the alternatives (for small sizes) as we don't need to allocate the
	/// full buffer - we only allocate what is required.
	pub fn encrypt_tiny(
		key: &Key,
		nonce: &Nonce,
		algorithm: Algorithm,
		bytes: &[u8],
		aad: Aad,
	) -> Result<Vec<u8>> {
		if bytes.len() >= BLOCK_LEN {
			return Err(Error::LengthMismatch);
		}

		let s = Self::new(key, nonce, algorithm)?;
		let mut buffer = Vec::with_capacity(bytes.len() + AEAD_TAG_LEN);
		buffer.extend_from_slice(bytes);
		s.encrypt_last_in_place(aad, &mut buffer)?;

		Ok(buffer)
	}
}

impl Decryptor {
	pub fn decrypt_key(
		key: &Key,
		algorithm: Algorithm,
		encrypted_key: &EncryptedKey,
		aad: Aad,
	) -> Result<Key> {
		Self::decrypt_tiny(
			key,
			encrypted_key.nonce(),
			algorithm,
			encrypted_key.inner(),
			aad,
		)
		.map(Key::try_from)
		.map_err(|_| Error::Decrypt)?
	}

	/// This is only for decrypting inputs < `BLOCK_LEN + AEAD_TAG_LEN`. For anything larger,
	/// see [`Decryptor::decrypt_bytes`] or [`Decryptor::decrypt_streams`].
	///
	/// It uses `decrypt_last_in_place` under the hood due to the input always being less than `BLOCK_LEN + AEAD_TAG_LEN`.
	///
	/// It's faster than the alternatives (for small sizes) as we don't need to allocate the
	/// full buffer - we only allocate what is required.
	pub fn decrypt_tiny(
		key: &Key,
		nonce: &Nonce,
		algorithm: Algorithm,
		bytes: &[u8],
		aad: Aad,
	) -> Result<Protected<Vec<u8>>> {
		if bytes.len() >= (BLOCK_LEN + AEAD_TAG_LEN) {
			return Err(Error::LengthMismatch);
		}

		let s = Self::new(key, nonce, algorithm)?;
		let mut buffer = Vec::with_capacity(bytes.len() + AEAD_TAG_LEN);
		buffer.extend_from_slice(bytes);
		s.decrypt_last_in_place(aad, &mut buffer)?;

		buffer.truncate(bytes.len() - AEAD_TAG_LEN);

		Ok(buffer.into())
	}
}

impl_stream!(
	Encryptor,
	Error::Encrypt,
	encrypt_next,
	encrypt_last,
	encrypt_last_in_place,
	EncryptorLE31,
	encrypt_streams,
	encrypt_streams_async,
	encrypt_bytes,
	Vec<u8>,
	BLOCK_LEN,
	Aes256GcmSiv,
	XChaCha20Poly1305
);

impl_stream!(
	Decryptor,
	Error::Decrypt,
	decrypt_next,
	decrypt_last,
	decrypt_last_in_place,
	DecryptorLE31,
	decrypt_streams,
	decrypt_streams_async,
	decrypt_bytes,
	Protected<Vec<u8>>,
	(BLOCK_LEN + AEAD_TAG_LEN),
	Aes256GcmSiv,
	XChaCha20Poly1305
);
