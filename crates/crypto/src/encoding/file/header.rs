use std::io::Cursor;

use super::{keyslot::Keyslot, object::HeaderObject, HeaderEncode, KEYSLOT_LIMIT, OBJECT_LIMIT};
use crate::{
	hashing::Hasher,
	primitives::AAD_HEADER_LEN,
	types::{
		Aad, Algorithm, DerivationContext, HashingAlgorithm, Key, MagicBytes, Nonce, Salt,
		SecretKey,
	},
	utils::ToArray,
	Error, Protected, Result,
};

pub struct Header {
	pub version: HeaderVersion,
	pub algorithm: Algorithm,
	pub nonce: Nonce,
	pub keyslots: Vec<Keyslot>,
	pub objects: Vec<HeaderObject>,
}

#[derive(Eq, PartialEq, Debug)]
pub enum HeaderVersion {
	V1,
}

impl std::fmt::Display for HeaderVersion {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::V1 => write!(f, "V1"),
		}
	}
}

impl Header {
	#[must_use]
	pub fn new(algorithm: Algorithm) -> Self {
		Self {
			version: HeaderVersion::V1,
			algorithm,
			nonce: Nonce::generate(algorithm),
			keyslots: vec![],
			objects: vec![],
		}
	}

	pub fn to_writer<W, const I: usize>(
		&self,
		writer: &mut W,
		magic_bytes: MagicBytes<I>,
	) -> Result<()>
	where
		W: std::io::Write,
	{
		let b = self.as_bytes()?;

		writer.write_all(magic_bytes.inner())?;

		// we're good here for up to 4096mib~ (headers should never be this large)
		writer.write_all(
			&(TryInto::<u32>::try_into(b.len()).map_err(|_| Error::Validity)?).to_le_bytes(),
		)?;
		writer.write_all(&b)?;

		Ok(())
	}

	#[cfg(feature = "tokio")]
	pub async fn to_writer_async<W, const I: usize>(
		&self,
		writer: &mut W,
		magic_bytes: MagicBytes<I>,
	) -> Result<()>
	where
		W: tokio::io::AsyncWriteExt + tokio::io::AsyncSeekExt + Unpin + Send,
	{
		let b = self.as_bytes()?;

		writer.write_all(magic_bytes.inner()).await?;

		// we're good here for up to 4096mib~ (headers should never be this large)
		writer
			.write_all(
				&(TryInto::<u32>::try_into(b.len()).map_err(|_| Error::Validity)?).to_le_bytes(),
			)
			.await?;
		writer.write_all(&b).await?;

		Ok(())
	}

	pub fn from_reader<R, const I: usize>(
		reader: &mut R,
		magic_bytes: MagicBytes<I>,
	) -> Result<(Self, Aad)>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = [0u8; I];
		reader.read_exact(&mut b)?;

		if &b != magic_bytes.inner() {
			return Err(Error::Validity);
		}

		let mut len = [0u8; 4];
		reader.read_exact(&mut len)?;
		let len = u32::from_le_bytes(len);

		let mut header_bytes = vec![0u8; len.try_into().map_err(|_| Error::Validity)?];
		reader.read_exact(&mut header_bytes)?;
		let h = Self::from_reader_raw(&mut Cursor::new(&header_bytes))?;

		Ok((h, Aad::Header(header_bytes[..AAD_HEADER_LEN].to_array()?)))
	}

	#[cfg(feature = "tokio")]
	pub async fn from_reader_async<R, const I: usize>(
		reader: &mut R,
		magic_bytes: MagicBytes<I>,
	) -> Result<(Self, Aad)>
	where
		R: tokio::io::AsyncReadExt + tokio::io::AsyncSeekExt + Unpin + Send,
	{
		let mut b = [0u8; I];
		reader.read_exact(&mut b).await?;

		if &b != magic_bytes.inner() {
			return Err(Error::Validity);
		}

		let mut len = [0u8; 4];
		reader.read_exact(&mut len).await?;
		let len = u32::from_le_bytes(len);

		let mut header_bytes = vec![0u8; len.try_into().map_err(|_| Error::Validity)?];
		reader.read_exact(&mut header_bytes).await?;
		let h = Self::from_reader_raw(&mut Cursor::new(&header_bytes))?;

		Ok((h, Aad::Header(header_bytes[..AAD_HEADER_LEN].to_array()?)))
	}

	#[must_use]
	pub fn generate_aad(&self) -> Aad {
		let mut o = [0u8; 38];
		o[..2].copy_from_slice(&[0xFA, 0xDA]);
		o[2..4].copy_from_slice(&self.version.as_bytes());
		o[4..6].copy_from_slice(&self.algorithm.as_bytes());
		o[6..38].copy_from_slice(&self.nonce.as_bytes());
		Aad::Header(o)
	}

	pub fn remove_keyslot(&mut self, index: usize) -> Result<()> {
		if index > self.keyslots.len() - 1 {
			return Err(Error::Validity);
		}

		self.keyslots.remove(index);
		Ok(())
	}

	pub fn decrypt_object(
		&self,
		name: &'static str,
		context: DerivationContext,
		master_key: &Key,
	) -> Result<Protected<Vec<u8>>> {
		let rhs = Hasher::blake3(name.as_bytes());

		self.objects
			.iter()
			.filter_map(|o| {
				o.identifier
					.decrypt_id(master_key, self.algorithm, context, self.generate_aad())
					.ok()
					.and_then(|i| (i == rhs).then_some(o))
			})
			// .cloned()
			.collect::<Vec<_>>()
			.first()
			.ok_or(Error::NoObjects)?
			.decrypt(self.algorithm, self.generate_aad(), master_key)
	}

	pub fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		hash_salt: Salt,
		hashed_password: &Key,
		master_key: &Key,
		context: DerivationContext,
	) -> Result<()> {
		if self.keyslots.len() + 1 > KEYSLOT_LIMIT {
			return Err(Error::TooManyKeyslots);
		}

		self.keyslots.push(Keyslot::new(
			self.algorithm,
			hashing_algorithm,
			hash_salt,
			hashed_password,
			master_key,
			self.generate_aad(),
			context,
		)?);

		Ok(())
	}

	pub fn add_object(
		&mut self,
		name: &'static str,
		context: DerivationContext,
		master_key: &Key,
		data: &[u8],
	) -> Result<()> {
		if self.objects.len() + 1 > OBJECT_LIMIT {
			return Err(Error::TooManyObjects);
		}

		let rhs = Hasher::blake3(name.as_bytes());

		if self
			.objects
			.iter()
			.filter_map(|o| {
				o.identifier
					.decrypt_id(master_key, self.algorithm, context, self.generate_aad())
					.ok()
					.map(|i| i == rhs)
			})
			.any(|x| x)
		{
			return Err(Error::TooManyObjects);
		}

		self.objects.push(HeaderObject::new(
			name,
			self.algorithm,
			master_key,
			context,
			self.generate_aad(),
			data,
		)?);
		Ok(())
	}

	pub fn decrypt_master_key(
		&self,
		keys: &[Key],
		context: DerivationContext,
	) -> Result<(Key, usize)> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		keys.iter()
			.enumerate()
			.find_map(|(i, k)| {
				self.keyslots.iter().find_map(|z| {
					z.decrypt(self.algorithm, k, self.generate_aad(), context)
						.ok()
						.map(|x| (x, i))
				})
			})
			.ok_or(Error::Decrypt)
	}

	pub fn decrypt_master_key_with_password(
		&self,
		password: &Protected<Vec<u8>>,
		context: DerivationContext,
	) -> Result<(Key, usize)> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		self.keyslots
			.iter()
			.enumerate()
			.find_map(|(i, z)| {
				let k = Hasher::hash_password(
					z.hashing_algorithm,
					password,
					z.hash_salt,
					&SecretKey::Null,
				)
				.ok()?;
				z.decrypt(self.algorithm, &k, self.generate_aad(), context)
					.ok()
					.map(|x| (x, i))
			})
			.ok_or(Error::Decrypt)
	}
}

#[cfg(test)]
mod tests {
	use crate::{ct::ConstantTimeEq, encoding::Header, types::MagicBytes};
	use std::io::{Cursor, Seek};

	const MAGIC_BYTES: MagicBytes<6> = MagicBytes::new(*b"crypto");

	#[test]
	fn encode_and_decode() {
		let mut w = Cursor::new(vec![]);
		let h_source = Header::new(crate::types::Algorithm::XChaCha20Poly1305);

		h_source.to_writer(&mut w, MAGIC_BYTES).unwrap();
		w.rewind().unwrap();

		let (h_read, aad) = Header::from_reader(&mut w, MAGIC_BYTES).unwrap();

		assert_eq!(w.into_inner().len(), 294);
		assert_eq!(h_source.algorithm, h_read.algorithm);
		assert_eq!(h_source.version, h_read.version);
		assert!(bool::from(h_source.nonce.ct_eq(&h_read.nonce)));
		assert!(bool::from(h_source.generate_aad().ct_eq(&aad)));
	}
}
