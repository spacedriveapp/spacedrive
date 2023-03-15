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

/// These are used to quickly and easily identify Spacedrive-encrypted files
///
/// These currently are set to "ballapp", plus the ASCII "ETX" code (`0x03`)
pub const MAGIC_BYTES: [u8; 8] = [0x62, 0x61, 0x6C, 0x6C, 0x61, 0x70, 0x70, 0x03];

// Can be expanded (not shrunk!) inconsequentially I believe
#[derive(bincode::Encode, bincode::Decode, Clone)]
pub enum HeaderObjectType {
	Metadata,
	PreviewMedia,
	Bytes,
}

#[async_trait::async_trait]
pub trait Header {
	fn serialize(&self) -> Result<Vec<u8>>;

	fn get_aad(&self) -> Aad;
	fn get_nonce(&self) -> Nonce;
	fn get_algorithm(&self) -> Algorithm;

	fn count_objects(&self) -> usize;
	fn count_keyslots(&self) -> usize;

	async fn decrypt_master_key(&self, keys: Vec<Key>) -> Result<Key>;
	async fn decrypt_master_key_with_password(&self, password: Protected<Vec<u8>>) -> Result<Key>;

	async fn decrypt_object(&self, index: usize, master_key: Key) -> Result<Protected<Vec<u8>>>;

	async fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		content_salt: Salt,
		hashed_key: Key,
		master_key: Key,
	) -> Result<()>;

	async fn add_object(
		&mut self,
		object_type: HeaderObjectType,
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
			pub fn new(
				version: FileHeaderVersion,
				algorithm: Algorithm,
			) -> Result<Self> {
				let header = match version {
					$(
						FileHeaderVersion::$version => $schema::new(algorithm)?,
					)*
				};

				Ok(Self { inner: Box::new(header), version })
			}

			/// This deserializes a header directly from a reader, and leaves said reader at the start of the encrypted data.
			pub async fn from_reader<R>(reader: &mut R) -> Result<Self>
			where
				R: AsyncReadExt + AsyncSeekExt + Unpin + Send,
			{
				let mut magic_bytes = [0u8; MAGIC_BYTES.len()];
				reader.read_exact(&mut magic_bytes).await?;

				if magic_bytes != MAGIC_BYTES {
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
	pub async fn write<W>(&self, writer: &mut W) -> Result<()>
	where
		W: AsyncWriteExt + Unpin + Send,
	{
		let bundle = HeaderBundle {
			version: self.version,
			bytes: self.inner.serialize()?,
		};

		let serialized_bundle = bincode::encode_to_vec(&bundle, bincode::config::standard())?;
		writer.write_all(&MAGIC_BYTES).await?;

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

	pub async fn decrypt_master_key(&self, keys: Vec<Key>) -> Result<Key> {
		self.inner.decrypt_master_key(keys).await
	}

	pub async fn decrypt_master_key_with_password(
		&self,
		password: Protected<Vec<u8>>,
	) -> Result<Key> {
		self.inner.decrypt_master_key_with_password(password).await
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
		index: usize,
		master_key: Key,
	) -> Result<Protected<Vec<u8>>> {
		self.inner.decrypt_object(index, master_key).await
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
	) -> Result<()> {
		self.inner
			.add_keyslot(hashing_algorithm, content_salt, hashed_key, master_key)
			.await
	}
}

#[cfg(test)]
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

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_keyslot(HASHING_ALGORITHM, content_salt, hashed_pw, mk)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		FileHeader::from_reader(&mut writer).await.unwrap();

		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	async fn serialize_and_deserialize_metadata() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::Metadata,
				mk.clone(),
				&bincode::encode_to_vec(&METADATA, bincode::config::standard()).unwrap(),
			)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer).await.unwrap();

		let bytes = header.decrypt_object(0, mk).await.unwrap();
		let (md, _): (Metadata, usize) =
			bincode::decode_from_slice(bytes.expose(), bincode::config::standard()).unwrap();

		assert_eq!(md, METADATA);
		assert!(header.count_objects() == 1);
		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	#[should_panic(expected = "Index")]
	async fn serialize_and_deserialize_metadata_wrong_index() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::Metadata,
				mk.clone(),
				&bincode::encode_to_vec(&METADATA, bincode::config::standard()).unwrap(),
			)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer).await.unwrap();

		header.decrypt_object(4, mk).await.unwrap();
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_one_object() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_object(HeaderObjectType::PreviewMedia, mk, &PVM_BYTES)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer).await.unwrap();

		assert!(header.count_objects() == 1);
		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_two_objects() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_object(HeaderObjectType::PreviewMedia, mk.clone(), &PVM_BYTES)
			.await
			.unwrap();

		header
			.add_object(HeaderObjectType::PreviewMedia, mk, &PVM_BYTES)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer).await.unwrap();

		assert!(header.count_objects() == 2);
		assert!(header.count_keyslots() == 1);
	}

	#[tokio::test]
	#[should_panic(expected = "TooManyKeyslots")]
	async fn serialize_and_deserialize_header_with_too_many_keyslots() {
		let mk = Key::generate();

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_keyslot(HASHING_ALGORITHM, Salt::generate(), Key::generate(), mk)
			.await
			.unwrap();
	}

	#[tokio::test]
	#[should_panic(expected = "TooManyObjects")]
	async fn serialize_and_deserialize_header_with_three_objects() {
		let mk = Key::generate();

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_object(HeaderObjectType::PreviewMedia, mk.clone(), &PVM_BYTES)
			.await
			.unwrap();

		header
			.add_object(
				HeaderObjectType::Metadata,
				mk.clone(),
				&bincode::encode_to_vec(&METADATA, bincode::config::standard()).unwrap(),
			)
			.await
			.unwrap();

		header
			.add_object(HeaderObjectType::Bytes, mk, &MAGIC_BYTES)
			.await
			.unwrap();
	}

	#[tokio::test]
	async fn serialize_and_deserialize_header_with_all() {
		let mk = Key::generate();
		let mut writer: Cursor<Vec<u8>> = Cursor::new(vec![]);

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM).unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
			)
			.await
			.unwrap();

		header
			.add_object(HeaderObjectType::PreviewMedia, mk.clone(), &PVM_BYTES)
			.await
			.unwrap();

		header
			.add_object(HeaderObjectType::Bytes, mk, &MAGIC_BYTES)
			.await
			.unwrap();

		header.write(&mut writer).await.unwrap();

		writer.rewind().await.unwrap();

		let header = FileHeader::from_reader(&mut writer).await.unwrap();

		assert!(header.count_objects() == 2);
		assert!(header.count_keyslots() == 2);
	}
}
