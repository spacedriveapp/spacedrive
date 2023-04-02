use super::{keyslot::Keyslot, object::HeaderObject, KEYSLOT_LIMIT, OBJECT_LIMIT};
use crate::{
	hashing::Hasher,
	types::{Aad, Algorithm, DerivationContext, HashingAlgorithm, Key, Nonce, Salt, SecretKey},
	Error, Protected, Result,
};

#[binrw::binrw]
#[bw(little)]
#[br(little)]
pub struct Header {
	pub version: HeaderVersion,
	pub aad: Aad,
	pub algorithm: Algorithm,
	pub nonce: Nonce,
	#[bw(try_calc(u8::try_from(keyslots.len())))]
	keyslots_len: u8,
	#[bw(try_calc(u8::try_from(objects.len())))]
	objects_len: u8,
	#[br(count = keyslots_len)]
	#[bw(pad_size_to = 202)]
	#[br(if (keyslots_len >= 1))]
	pub keyslots: Vec<Keyslot>,
	#[br(count = objects_len)]
	pub objects: Vec<HeaderObject>,
}

#[binrw::binrw]
#[br(repr = u8)]
#[bw(repr = u8)]
pub enum HeaderVersion {
	V1,
}

impl Header {
	// TODO(brxken128): make the AAD not static
	// should be brought in from the raw file bytes but bincode makes that harder
	// as the first 32~ bytes of the file *may* change
	#[must_use]
	pub fn new(algorithm: Algorithm) -> Self {
		Self {
			version: HeaderVersion::V1,
			aad: Aad::generate(),
			algorithm,
			nonce: Nonce::generate(algorithm),
			keyslots: vec![],
			objects: vec![],
		}
	}

	pub fn from_reader() -> Result<(Self, Aad)> {
		todo!()
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
		master_key: Key,
	) -> Result<Protected<Vec<u8>>> {
		let rhs = Hasher::blake3(name.as_bytes());

		self.objects
			.iter()
			.filter_map(|o| {
				o.identifier
					.decrypt_id(master_key.clone(), self.algorithm, context, self.aad)
					.ok()
					.and_then(|i| (i == rhs).then_some(o))
			})
			// .cloned()
			.collect::<Vec<_>>()
			.first()
			.ok_or(Error::NoObjects)?
			.decrypt(self.algorithm, self.aad, master_key)
	}

	pub fn add_keyslot(
		&mut self,
		hashing_algorithm: HashingAlgorithm,
		hash_salt: Salt,
		hashed_password: Key,
		master_key: Key,
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
			self.aad,
			context,
		)?);

		Ok(())
	}

	pub fn add_object(
		&mut self,
		name: &'static str,
		context: DerivationContext,
		master_key: Key,
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
					.decrypt_id(master_key.clone(), self.algorithm, context, self.aad)
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
			self.aad,
			data,
		)?);
		Ok(())
	}

	#[allow(clippy::needless_pass_by_value)]
	pub fn decrypt_master_key(
		&self,
		keys: Vec<Key>,
		context: DerivationContext,
	) -> Result<(Key, usize)> {
		if self.keyslots.is_empty() {
			return Err(Error::NoKeyslots);
		}

		keys.iter()
			.enumerate()
			.find_map(|(i, k)| {
				self.keyslots.iter().find_map(|z| {
					z.decrypt(self.algorithm, k.clone(), self.aad, context)
						.ok()
						.map(|x| (x, i))
				})
			})
			.ok_or(Error::Decrypt)
	}

	#[allow(clippy::needless_pass_by_value)]
	pub fn decrypt_master_key_with_password(
		&self,
		password: Protected<Vec<u8>>,
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
					password.clone(),
					z.hash_salt,
					SecretKey::Null,
				)
				.ok()?;
				z.decrypt(self.algorithm, k, self.aad, context)
					.ok()
					.map(|x| (x, i))
			})
			.ok_or(Error::Decrypt)
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
	use crate::encoding::Header;
	use binrw::BinWrite;
	use std::io::Cursor;

	#[test]
	fn t() {
		let mut w = Cursor::new(vec![]);
		Header::new(crate::types::Algorithm::XChaCha20Poly1305)
			.write_le(&mut w)
			.unwrap();
		assert_eq!(w.into_inner().len(), 258);
	}
}
