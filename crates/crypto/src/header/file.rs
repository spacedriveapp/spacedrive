//! This module contains a standard file header, and the functions needed to serialize/deserialize it.
//!
//! # Examples
//!
//! ```rust,ignore
//! let password = Protected::new(b"password".to_vec());
//!
//! let mut writer = File::create("test.encrypted").unwrap();
//!
//! // This needs to be generated here, otherwise we won't have access to it for encryption
//! let master_key = generate_master_key();
//!
//! // Create a keyslot to be added to the header
//! let mut keyslots: Vec<Keyslot> = Vec::new();
//! keyslots.push(
//!     Keyslot::new(
//!         KeyslotVersion::V1,
//!         ALGORITHM,
//!         HASHING_ALGORITHM,
//!         password,
//!         &master_key,
//!     )
//!     .unwrap(),
//! );
//!
//! // Create the header for the encrypted file
//! let header = FileHeader::new(FileHeaderVersion::V1, ALGORITHM, keyslots, None, None);
//!
//! // Write the header to the file
//! header.write(&mut writer).unwrap();
//! ```

use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::{
	types::{Aad, Algorithm, HashingAlgorithm, Key, Nonce, Salt},
	Error, Protected, Result,
};

use super::schema::FileHeader001;

#[derive(Clone, PartialEq, Eq, bincode::Encode, bincode::Decode)]
pub struct HeaderObjectType([u8; 32]);

impl HeaderObjectType {
	#[must_use]
	pub fn new(identifier: &str, context: &str) -> Self {
		Self(blake3::derive_key(context, identifier.as_bytes()))
	}
}

#[async_trait::async_trait]
pub trait Header {
	fn serialize(&self) -> Result<Vec<u8>>;

	fn get_aad(&self) -> Aad;
	fn get_nonce(&self) -> Nonce;
	fn get_algorithm(&self) -> Algorithm;

	fn count_objects(&self) -> usize;
	fn count_keyslots(&self) -> usize;

	async fn decrypt_master_key(&self, keys: Vec<Key>, context: &str) -> Result<Key>;
	async fn decrypt_master_key_with_password(
		&self,
		password: Protected<Vec<u8>>,
		context: &str,
	) -> Result<Key>;

	async fn decrypt_object(
		&self,
		identifier: HeaderObjectType,
		master_key: Key,
	) -> Result<Protected<Vec<u8>>>;

	async fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		content_salt: Salt,
		hashed_key: Key,
		master_key: Key,
		context: &str,
	) -> Result<()>;

	async fn add_object(
		&mut self,
		identifier: HeaderObjectType,
		master_key: Key,
		data: &[u8],
	) -> Result<()>;
}

/// This header is primarily used for encrypting/decrypting single files.
///
/// You may optionally attach additional objects to this header, and they will be accessible (and decryptable) once the header has been deserialized.
///
/// This contains everything necessary for decryption, and the entire header can be shared with no worries (provided a suitable password was selected by the user).
pub struct FileHeader {
	inner: Box<dyn Header + Send + Sync>,
	version: FileHeaderVersion,
}

/// This is the on-disk wrapper around headers and their versions.
///
/// By isolating the two, we know which schema we need to re-build for a given version.
#[derive(bincode::Encode, bincode::Decode)]
pub struct HeaderBundle {
	pub version: FileHeaderVersion,
	pub bytes: Vec<u8>,
}

macro_rules! generate_header_versions {
	(
		$default:tt, $default_schema:ident,
		$(($version:tt, $schema:ident, $keyslot_schema:ident)),*
	) => {
		/// This defines the latest/default file header version
		pub const LATEST_FILE_HEADER: FileHeaderVersion = FileHeaderVersion::$default;

		// This defines all possible file header versions
		#[derive(Clone, Copy, bincode::Encode, bincode::Decode)]
		pub enum FileHeaderVersion {
			$(
				$version,
			)*
		}

		impl std::fmt::Display for FileHeaderVersion {
			fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
				match *self {
					$(
						Self::$version => write!(f, stringify!($version)),
					)*
				}
			}
		}

		impl FileHeader {
			#[must_use]
			pub fn new(
				version: FileHeaderVersion,
				algorithm: Algorithm,
			) -> Self {
				let header = match version {
					$(
						FileHeaderVersion::$version => $schema::new(algorithm),
					)*
				};

				Self { inner: Box::new(header), version }
			}

			/// This deserializes a header directly from a reader, and leaves said reader at the start of the encrypted data.
			pub async fn from_reader<R, const I: usize>(reader: &mut R, magic_bytes: [u8; I]) -> Result<Self>
			where
				R: AsyncReadExt + AsyncSeekExt + Unpin + Send,
			{
				let mut mb = [0u8; I];
				reader.read_exact(&mut mb).await?;

				if mb != magic_bytes {
					return Err(Error::Serialization);
				}

				let mut header_size = [0u8; 8];
				reader.read_exact(&mut header_size).await?;
				let header_size = u64::from_le_bytes(header_size);

				#[allow(clippy::cast_possible_truncation)]
				let mut header_bytes = vec![0u8; header_size as usize];
				reader.read_exact(&mut header_bytes).await?;

				let (bundle, _): (HeaderBundle, usize) = bincode::decode_from_slice(&header_bytes, bincode::config::standard())?;
				let header: Box<dyn Header + Send + Sync> = match bundle.version {
					$(
						FileHeaderVersion::$version => Box::new(bincode::decode_from_slice::<$schema, bincode::config::Configuration>(&bundle.bytes, bincode::config::standard())?.0),
					)*
				};

				Ok((Self {
					inner: header,
					version: bundle.version,
				}))
			}
		}
	};
}

generate_header_versions!(V001, FileHeader001, (V001, FileHeader001, Keyslot001));

impl FileHeader {
	/// This is a helper function to serialize and write a header to a file.
	pub async fn write<W, const I: usize>(&self, writer: &mut W, magic_bytes: [u8; I]) -> Result<()>
	where
		W: AsyncWriteExt + Unpin + Send,
	{
		let bundle = HeaderBundle {
			version: self.version,
			bytes: self.inner.serialize()?,
		};

		let serialized_bundle = bincode::encode_to_vec(&bundle, bincode::config::standard())?;
		writer.write_all(&magic_bytes).await?;

		writer
			.write_all(&(serialized_bundle.len() as u64).to_le_bytes())
			.await?;

		writer.write_all(&serialized_bundle).await?;

		Ok(())
	}

	#[must_use]
	pub fn get_aad(&self) -> Aad {
		self.inner.get_aad()
	}

	#[must_use]
	pub fn get_algorithm(&self) -> Algorithm {
		self.inner.get_algorithm()
	}

	#[must_use]
	pub fn get_nonce(&self) -> Nonce {
		self.inner.get_nonce()
	}

	#[must_use]
	pub const fn get_version(&self) -> FileHeaderVersion {
		self.version
	}

	pub async fn decrypt_master_key(&self, keys: Vec<Key>, context: &str) -> Result<Key> {
		self.inner.decrypt_master_key(keys, context).await
	}

	pub async fn decrypt_master_key_with_password(
		&self,
		password: Protected<Vec<u8>>,
		context: &str,
	) -> Result<Key> {
		self.inner
			.decrypt_master_key_with_password(password, context)
			.await
	}

	#[must_use]
	pub fn count_objects(&self) -> usize {
		self.inner.count_objects()
	}

	#[must_use]
	pub fn count_keyslots(&self) -> usize {
		self.inner.count_keyslots()
	}

	pub async fn decrypt_object(
		&self,
		object_type: HeaderObjectType,
		master_key: Key,
	) -> Result<Protected<Vec<u8>>> {
		self.inner.decrypt_object(object_type, master_key).await
	}

	pub async fn add_object(
		&mut self,
		object_type: HeaderObjectType,
		master_key: Key,
		data: &[u8],
	) -> Result<()> {
		self.inner.add_object(object_type, master_key, data).await
	}

	pub async fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		content_salt: Salt,
		hashed_key: Key,
		master_key: Key,
		context: &str,
	) -> Result<()> {
		self.inner
			.add_keyslot(
				hashing_algorithm,
				content_salt,
				hashed_key,
				master_key,
				context,
			)
			.await
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use super::*;
	use crate::types::{HashingAlgorithm, Params, Salt};
	use std::io::Cursor;

	#[derive(Clone, Eq, PartialEq, Debug, bincode::Encode, bincode::Decode)]
	struct Metadata {
		count: usize,
		enabled: bool,
	}

	const ALGORITHM: Algorithm = Algorithm::XChaCha20Poly1305;
	const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::Argon2id(Params::Standard);

	const MAGIC_BYTES: [u8; 6] = *b"crypto";

	const FILE_HEADER_CONTEXT: &str = "spacedrive 2023-03-16 18:27:55 header keyslot tests";
	const OBJECT_IDENTIFIER_CONTEXT: &str = "spacedrive 2023-03-16 18:10:47 header object tests";

	const PVM_BYTES: [u8; 4] = [0x01, 0x02, 0x03, 0x04];
	const METADATA: Metadata = Metadata {
		count: 43948,
		enabled: true,
	};

	#[tokio::test]
	async fn serialize_and_deserialize_header() {
		let mk = Key::generate();
		let content_salt = Salt::generate();
		let hashed_pw = Key::generate(); // not hashed, but that'd be expensive

		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw,
				mk,
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().await.unwrap();

		FileHeader::from_reader(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_two_keyslots() {
		let mk = Key::generate();
		let content_salt = Salt::generate();
		let hashed_pw = Key::generate(); // not hashed, but that'd be expensive

		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw.clone(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw,
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().await.unwrap();

		FileHeader::from_reader(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		assert!(header.count_keyslots() == 2);
	}

	#[tokio::test]
	async fn serialize_and_deserialize_metadata() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("Metadata", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
				&bincode::encode_to_vec(&METADATA, bincode::config::standard()).unwrap(),
			)
			.await
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		let bytes = header
			.decrypt_object(
				HeaderObjectType::new("Metadata", OBJECT_IDENTIFIER_CONTEXT),
				mk,
			)
			.await
			.unwrap();

		let (md, _): (Metadata, usize) =
			bincode::decode_from_slice(bytes.expose(), bincode::config::standard()).unwrap();

		assert_eq!(md, METADATA);
		assert!(header.count_objects() == 1);
		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	#[should_panic(expected = "NoObjects")]
	async fn serialize_and_deserialize_metadata_bad_identifier() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("Metadata", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
				&bincode::encode_to_vec(&METADATA, bincode::config::standard()).unwrap(),
			)
			.await
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		header
			.decrypt_object(
				HeaderObjectType::new("PreviewMedia", OBJECT_IDENTIFIER_CONTEXT),
				mk,
			)
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_one_object() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("PreviewMedia", OBJECT_IDENTIFIER_CONTEXT),
				mk,
				&PVM_BYTES,
			)
			.await
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		assert!(header.count_objects() == 1);
		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_two_objects() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("PreviewMedia", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
				&PVM_BYTES,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("MagicBytes", OBJECT_IDENTIFIER_CONTEXT),
				mk,
				&PVM_BYTES,
			)
			.await
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		assert!(header.count_objects() == 2);
		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	#[should_panic(expected = "TooManyKeyslots")]
	async fn serialize_and_deserialize_header_with_too_many_keyslots() {
		let mk = Key::generate();

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk,
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();
	}

	#[tokio::test]
	#[should_panic(expected = "TooManyObjects")]
	async fn serialize_and_deserialize_header_with_three_objects() {
		let mk = Key::generate();

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_object(
				HeaderObjectType::new("PreviewMedia", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
				&PVM_BYTES,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("Metadata", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
				&bincode::encode_to_vec(&METADATA, bincode::config::standard()).unwrap(),
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("MagicBytes", OBJECT_IDENTIFIER_CONTEXT),
				mk,
				&MAGIC_BYTES,
			)
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_all() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				FILE_HEADER_CONTEXT,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("PreviewMedia", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
				&PVM_BYTES,
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::new("MagicBytes", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
				&MAGIC_BYTES,
			)
			.await
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		assert!(header.count_objects() == 2);
		assert!(header.count_keyslots() == 2);

		let preview_media = header
			.decrypt_object(
				HeaderObjectType::new("PreviewMedia", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
			)
			.await
			.unwrap();

		let magic = header
			.decrypt_object(
				HeaderObjectType::new("MagicBytes", OBJECT_IDENTIFIER_CONTEXT),
				mk.clone(),
			)
			.await
			.unwrap();

		assert_eq!(preview_media.expose(), &PVM_BYTES);
		assert_eq!(magic.expose(), &MAGIC_BYTES);
	}
}
