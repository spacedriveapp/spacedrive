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

use std::io::{Read, Seek, Write};

#[cfg(feature = "async")]
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::{
	encoding,
	types::{Aad, Algorithm, DerivationContext, HashingAlgorithm, Key, MagicBytes, Nonce, Salt},
	Error, Protected, Result,
};

use super::schema::FileHeader001;

#[derive(Clone)]
pub struct HeaderObjectName(&'static str);

impl HeaderObjectName {
	#[must_use]
	pub const fn new(name: &'static str) -> Self {
		Self(name)
	}

	#[must_use]
	pub const fn inner(&self) -> &[u8] {
		self.0.as_bytes()
	}
}

pub trait Header {
	fn serialize(&self) -> Result<Vec<u8>>;
	fn get_aad(&self) -> Aad;
	fn get_nonce(&self) -> Nonce;
	fn get_algorithm(&self) -> Algorithm;
	fn count_objects(&self) -> usize;
	fn count_keyslots(&self) -> usize;
	fn decrypt_master_key(&self, keys: Vec<Key>, context: DerivationContext) -> Result<Key>;
	fn decrypt_master_key_with_password(
		&self,
		password: Protected<Vec<u8>>,
		context: DerivationContext,
	) -> Result<Key>;
	fn decrypt_object(
		&self,
		name: HeaderObjectName,
		context: DerivationContext,
		master_key: Key,
	) -> Result<Protected<Vec<u8>>>;
	fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		content_salt: Salt,
		hashed_password: Key,
		master_key: Key,
		context: DerivationContext,
	) -> Result<()>;
	fn add_object(
		&mut self,
		name: HeaderObjectName,
		context: DerivationContext,
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
			pub fn from_reader<R, const I: usize>(reader: &mut R, magic_bytes: MagicBytes<I>) -> Result<Self>
			where
				R: Read + Seek,
			{
				let mut mb = [0u8; I];
				reader.read_exact(&mut mb)?;

				if &mb != magic_bytes.inner() {
					return Err(Error::MagicByteMismatch);
				}

				let mut header_size = [0u8; 8];
				reader.read_exact(&mut header_size)?;
				let header_size = u64::from_le_bytes(header_size);

				#[allow(clippy::cast_possible_truncation)]
				let mut header_bytes = vec![0u8; header_size as usize];
				reader.read_exact(&mut header_bytes)?;

				let bundle: HeaderBundle = encoding::decode(&header_bytes)?;
				let header: Box<dyn Header + Send + Sync> = match bundle.version {
					$(
						FileHeaderVersion::$version => Box::new(encoding::decode::<$schema>(&bundle.bytes)?),
					)*
				};

				Ok((Self {
					inner: header,
					version: bundle.version,
				}))
			}

			/// This deserializes a header directly from a reader, and leaves said reader at the start of the encrypted data.
			#[cfg(feature = "async")]
			pub async fn from_reader_async<R, const I: usize>(reader: &mut R, magic_bytes: MagicBytes<I>) -> Result<Self>
			where
				R: AsyncReadExt + AsyncSeekExt + Unpin + Send,
			{
				let mut mb = [0u8; I];
				reader.read_exact(&mut mb).await?;

				if &mb != magic_bytes.inner() {
					return Err(Error::MagicByteMismatch);
				}

				let mut header_size = [0u8; 8];
				reader.read_exact(&mut header_size).await?;
				let header_size = u64::from_le_bytes(header_size);

				#[allow(clippy::cast_possible_truncation)]
				let mut header_bytes = vec![0u8; header_size as usize];
				reader.read_exact(&mut header_bytes).await?;

				let bundle: HeaderBundle = encoding::decode(&header_bytes)?;
				let header: Box<dyn Header + Send + Sync> = match bundle.version {
					$(
						FileHeaderVersion::$version => Box::new(encoding::decode::<$schema>(&bundle.bytes)?),
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
	#[allow(clippy::needless_pass_by_value)]
	pub fn write<W, const I: usize>(&self, writer: &mut W, magic_bytes: MagicBytes<I>) -> Result<()>
	where
		W: Write,
	{
		let bundle = HeaderBundle {
			version: self.version,
			bytes: self.inner.serialize()?,
		};

		let serialized_bundle = encoding::encode(&bundle)?;
		writer.write_all(magic_bytes.inner())?;

		writer.write_all(&(serialized_bundle.len() as u64).to_le_bytes())?;

		writer.write_all(&serialized_bundle)?;

		Ok(())
	}

	/// This is a helper function to serialize and write a header to a file.
	#[cfg(feature = "async")]
	pub async fn write_async<W, const I: usize>(
		&self,
		writer: &mut W,
		magic_bytes: MagicBytes<I>,
	) -> Result<()>
	where
		W: AsyncWriteExt + Unpin + Send,
	{
		let bundle = HeaderBundle {
			version: self.version,
			bytes: self.inner.serialize()?,
		};

		let serialized_bundle = encoding::encode(&bundle)?;
		writer.write_all(magic_bytes.inner()).await?;

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

	pub fn decrypt_master_key(&self, keys: Vec<Key>, context: DerivationContext) -> Result<Key> {
		self.inner.decrypt_master_key(keys, context)
	}

	pub fn decrypt_master_key_with_password(
		&self,
		password: Protected<Vec<u8>>,
		context: DerivationContext,
	) -> Result<Key> {
		self.inner
			.decrypt_master_key_with_password(password, context)
	}

	#[must_use]
	pub fn count_objects(&self) -> usize {
		self.inner.count_objects()
	}

	#[must_use]
	pub fn count_keyslots(&self) -> usize {
		self.inner.count_keyslots()
	}

	pub fn decrypt_object(
		&self,
		name: HeaderObjectName,
		context: DerivationContext,
		master_key: Key,
	) -> Result<Protected<Vec<u8>>> {
		self.inner.decrypt_object(name, context, master_key)
	}

	pub fn add_object(
		&mut self,
		name: HeaderObjectName,
		context: DerivationContext,
		master_key: Key,
		data: &[u8],
	) -> Result<()> {
		self.inner.add_object(name, context, master_key, data)
	}

	pub fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		content_salt: Salt,
		hashed_password: Key,
		master_key: Key,
		context: DerivationContext,
	) -> Result<()> {
		self.inner.add_keyslot(
			hashing_algorithm,
			content_salt,
			hashed_password,
			master_key,
			context,
		)
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use std::io::{Cursor, Seek};
	use subtle::ConstantTimeEq;

	use crate::{
		assert_ct_eq,
		header::{FileHeader, HeaderObjectName},
		keys::Hasher,
		primitives::LATEST_FILE_HEADER,
		types::{Algorithm, DerivationContext, HashingAlgorithm, Key, MagicBytes, Salt},
	};

	const MAGIC_BYTES: MagicBytes<6> = MagicBytes::new(*b"crypto");

	// don't do this in production code - use separate contexts for keys and objects
	const CONTEXT: DerivationContext =
		DerivationContext::new("crypto 2023-03-20 20:12:42 global test context");

	const ALGORITHM: Algorithm = Algorithm::default();
	const HASHING_ALGORITHM: HashingAlgorithm = HashingAlgorithm::default();

	const PASSWORD: [u8; 16] = [5u8; 16];

	const OBJECT1_NAME: HeaderObjectName = HeaderObjectName::new("Object1");
	const OBJECT2_NAME: HeaderObjectName = HeaderObjectName::new("Object2");

	const OBJECT1_DATA: [u8; 16] = [4u8; 16];
	const OBJECT2_DATA: [u8; 16] = [5u8; 16];

	#[test]
	fn serialize_and_deserialize() {
		let mk = Key::generate();
		let content_salt = Salt::generate();
		let hashed_pw = Key::generate(); // not hashed, but that'd be expensive

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw.clone(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();
		let decrypted_mk = header.decrypt_master_key(vec![hashed_pw], CONTEXT).unwrap();

		assert_eq!(header.count_keyslots(), 1);
		assert_eq!(header.count_objects(), 0);
		assert_eq!(header.get_algorithm(), ALGORITHM);
		assert_ct_eq!(decrypted_mk, mk);
	}

	#[test]
	fn serialize_and_deserialize_no_extras() {
		let mut writer = Cursor::new(vec![]);
		let header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();

		assert_eq!(header.count_keyslots(), 0);
		assert_eq!(header.count_objects(), 0);
		assert_eq!(header.get_algorithm(), ALGORITHM);
	}

	#[test]
	fn serialize_and_deserialize_no_keyslot_one_object() {
		let mk = Key::generate();

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();

		let object1 = header.decrypt_object(OBJECT1_NAME, CONTEXT, mk).unwrap();

		assert_eq!(header.count_keyslots(), 0);
		assert_eq!(header.count_objects(), 1);
		assert_eq!(header.get_algorithm(), ALGORITHM);
		assert_eq!(object1.expose(), &OBJECT1_DATA);
	}

	#[test]
	fn serialize_and_deserialize_no_keyslot_two_objects() {
		let mk = Key::generate();

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header
			.add_object(OBJECT2_NAME, CONTEXT, mk.clone(), &OBJECT2_DATA)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();

		let object1 = header
			.decrypt_object(OBJECT1_NAME, CONTEXT, mk.clone())
			.unwrap();
		let object2 = header.decrypt_object(OBJECT2_NAME, CONTEXT, mk).unwrap();

		assert_eq!(header.count_keyslots(), 0);
		assert_eq!(header.count_objects(), 2);
		assert_eq!(header.get_algorithm(), ALGORITHM);
		assert_eq!(object1.expose(), &OBJECT1_DATA);
		assert_eq!(object2.expose(), &OBJECT2_DATA);
	}

	#[test]
	#[should_panic(expected = "NoKeyslots")]
	fn serialize_and_deserialize_no_keyslot_attempt_decrypt() {
		let mut writer = Cursor::new(vec![]);
		let header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();
		header
			.decrypt_master_key(vec![Key::generate()], CONTEXT)
			.unwrap();
	}

	#[test]
	fn serialize_and_deserialize_with_password() {
		let mk = Key::generate();
		let content_salt = Salt::generate();

		let hashed_pw = Hasher::hash_password(
			HASHING_ALGORITHM,
			PASSWORD.to_vec().into(),
			content_salt,
			None,
		)
		.unwrap();

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw,
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();
		let decrypted_mk = header
			.decrypt_master_key_with_password(PASSWORD.to_vec().into(), CONTEXT)
			.unwrap();

		assert_eq!(header.count_keyslots(), 1);
		assert_eq!(header.count_objects(), 0);
		assert_ct_eq!(decrypted_mk, mk);
	}

	#[test]
	#[should_panic(expected = "Decrypt")]
	fn serialize_and_deserialize_with_password_incorrect() {
		let mk = Key::generate();
		let content_salt = Salt::generate();

		let hashed_pw = Hasher::hash_password(
			HASHING_ALGORITHM,
			PASSWORD.to_vec().into(),
			content_salt,
			None,
		)
		.unwrap();

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(HASHING_ALGORITHM, content_salt, hashed_pw, mk, CONTEXT)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();
		header
			.decrypt_master_key_with_password(vec![1u8; 8].into(), CONTEXT)
			.unwrap();
	}

	#[test]
	#[should_panic(expected = "MagicByteMismatch")]
	fn serialize_and_deserialize_with_bad_magic_bytes() {
		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				Key::generate(),
				CONTEXT,
			)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		FileHeader::from_reader(&mut writer, MagicBytes::new([0u8; 4])).unwrap();
	}

	#[test]
	fn serialize_and_deserialize_with_two_keyslots() {
		let mk = Key::generate();
		let content_salt = Salt::generate();
		let hashed_pw = Key::generate(); // not hashed, but that'd be expensive
		let hashed_pw2 = Key::generate();

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw.clone(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				content_salt,
				hashed_pw2.clone(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();
		let decrypted_mk = header.decrypt_master_key(vec![hashed_pw], CONTEXT).unwrap();
		let decrypted_mk2 = header
			.decrypt_master_key(vec![hashed_pw2], CONTEXT)
			.unwrap();

		assert_eq!(header.count_keyslots(), 2);
		assert_eq!(header.count_objects(), 0);
		assert_ct_eq!(decrypted_mk, mk);
		assert_ct_eq!(decrypted_mk2, mk);
	}

	#[test]
	fn serialize_and_deserialize_with_object() {
		let mk = Key::generate();
		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();

		let object1 = header.decrypt_object(OBJECT1_NAME, CONTEXT, mk).unwrap();

		assert_eq!(header.count_keyslots(), 1);
		assert_eq!(header.count_objects(), 1);
		assert_eq!(object1.expose(), &OBJECT1_DATA);
	}

	#[test]
	#[should_panic(expected = "NoObjects")]
	fn serialize_and_deserialize_with_object_bad_identifier() {
		let mk = Key::generate();
		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();

		header
			.decrypt_object(HeaderObjectName::new("NonExistent"), CONTEXT, mk)
			.unwrap();
	}

	#[test]
	fn serialize_and_deserialize_with_two_objects() {
		let mk = Key::generate();
		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header
			.add_object(OBJECT2_NAME, CONTEXT, mk.clone(), &OBJECT2_DATA)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();

		let object1 = header
			.decrypt_object(OBJECT1_NAME, CONTEXT, mk.clone())
			.unwrap();

		let object2 = header.decrypt_object(OBJECT2_NAME, CONTEXT, mk).unwrap();

		assert_eq!(header.count_keyslots(), 1);
		assert_eq!(header.count_objects(), 2);
		assert_eq!(object1.expose(), &OBJECT1_DATA);
		assert_eq!(object2.expose(), &OBJECT2_DATA);
	}

	#[test]
	#[should_panic(expected = "TooManyObjects")]
	fn serialize_and_deserialize_with_two_objects_same_name() {
		let mk = Key::generate();

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk, &OBJECT1_DATA)
			.unwrap();
	}

	#[test]
	#[should_panic(expected = "TooManyKeyslots")]
	fn serialize_and_deserialize_with_too_many_keyslots() {
		let mk = Key::generate();

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				Key::generate(),
				mk,
				CONTEXT,
			)
			.unwrap();
	}

	#[test]
	#[should_panic(expected = "TooManyObjects")]
	fn serialize_and_deserialize_with_three_objects() {
		let mk = Key::generate();

		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header
			.add_object(OBJECT2_NAME, CONTEXT, mk.clone(), &OBJECT2_DATA)
			.unwrap();

		header
			.add_object(HeaderObjectName::new("dg"), CONTEXT, mk, &[0u8; 4])
			.unwrap();
	}

	#[test]
	fn serialize_and_deserialize_with_all() {
		let mk = Key::generate();
		let hashed_pw = Key::generate(); // not hashed, but that'd be expensive
		let hashed_pw2 = Key::generate();

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				hashed_pw.clone(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				hashed_pw2.clone(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header
			.add_object(OBJECT2_NAME, CONTEXT, mk.clone(), &OBJECT2_DATA)
			.unwrap();

		header.write(&mut writer, MAGIC_BYTES).unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader(&mut writer, MAGIC_BYTES).unwrap();

		let decrypted_mk = header.decrypt_master_key(vec![hashed_pw], CONTEXT).unwrap();
		let decrypted_mk2 = header
			.decrypt_master_key(vec![hashed_pw2], CONTEXT)
			.unwrap();

		let object1 = header
			.decrypt_object(OBJECT1_NAME, CONTEXT, mk.clone())
			.unwrap();
		let object2 = header
			.decrypt_object(OBJECT2_NAME, CONTEXT, mk.clone())
			.unwrap();

		assert_eq!(header.count_objects(), 2);
		assert_eq!(header.count_keyslots(), 2);
		assert_eq!(object1.expose(), &OBJECT1_DATA);
		assert_eq!(object2.expose(), &OBJECT2_DATA);
		assert_ct_eq!(decrypted_mk, mk);
		assert_ct_eq!(decrypted_mk2, mk);
	}

	#[cfg(feature = "async")]
	#[tokio::test]
	async fn serialize_and_deserialize_with_all_async() {
		let mk = Key::generate();
		let hashed_pw = Key::generate(); // not hashed, but that'd be expensive
		let hashed_pw2 = Key::generate();

		let mut writer = Cursor::new(vec![]);
		let mut header = FileHeader::new(LATEST_FILE_HEADER, ALGORITHM);

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				hashed_pw.clone(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_keyslot(
				HASHING_ALGORITHM,
				Salt::generate(),
				hashed_pw2.clone(),
				mk.clone(),
				CONTEXT,
			)
			.unwrap();

		header
			.add_object(OBJECT1_NAME, CONTEXT, mk.clone(), &OBJECT1_DATA)
			.unwrap();

		header
			.add_object(OBJECT2_NAME, CONTEXT, mk.clone(), &OBJECT2_DATA)
			.unwrap();

		header.write_async(&mut writer, MAGIC_BYTES).await.unwrap();

		writer.rewind().unwrap();

		let header = FileHeader::from_reader_async(&mut writer, MAGIC_BYTES)
			.await
			.unwrap();

		let decrypted_mk = header.decrypt_master_key(vec![hashed_pw], CONTEXT).unwrap();
		let decrypted_mk2 = header
			.decrypt_master_key(vec![hashed_pw2], CONTEXT)
			.unwrap();

		let object1 = header
			.decrypt_object(OBJECT1_NAME, CONTEXT, mk.clone())
			.unwrap();
		let object2 = header
			.decrypt_object(OBJECT2_NAME, CONTEXT, mk.clone())
			.unwrap();

		assert_eq!(header.count_objects(), 2);
		assert_eq!(header.count_keyslots(), 2);
		assert_eq!(object1.expose(), &OBJECT1_DATA);
		assert_eq!(object2.expose(), &OBJECT2_DATA);
		assert_ct_eq!(decrypted_mk, mk);
		assert_ct_eq!(decrypted_mk2, mk);
	}
}
