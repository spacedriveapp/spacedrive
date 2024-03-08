use std::mem;

use crate::{
	primitives::{
		AES_256_GCM_SIV_NONCE_LEN, ENCRYPTED_KEY_LEN, SALT_LEN, XCHACHA20_POLY1305_NONCE_LEN,
	},
	types::{Algorithm, EncryptedKey, HashingAlgorithm, Nonce, Params, Salt},
	utils::ToArray,
	Error, Result,
};

use self::{
	header::HeaderVersion,
	keyslot::Keyslot,
	object::{HeaderObject, HeaderObjectIdentifier},
};

use super::Header;

pub mod header;
pub mod keyslot;
pub mod object;

const KEYSLOT_LIMIT: usize = 2;
const OBJECT_LIMIT: usize = 2;

pub trait HeaderEncode {
	const OUTPUT_LEN: usize;
	type Identifier;
	type Output: Default;

	fn as_bytes(&self) -> Self::Output;

	fn from_bytes(b: Self::Output) -> Result<Self>
	where
		Self: Sized;

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		Self: Sized,
		R: std::io::Read + std::io::Seek; // make this a provided method eventually via `hybrid-array`?
}

// TODO(brxken128): convert as many of these as possible to vec
// also define the identifiers as consts where possble?
// typenum/hybrid-array/generic-array too

impl HeaderEncode for Params {
	const OUTPUT_LEN: usize = 1;
	type Identifier = u8;
	type Output = u8;

	fn as_bytes(&self) -> Self::Output {
		match self {
			Self::Standard => 18u8,
			Self::Hardened => 39u8,
			Self::Paranoid => 56u8,
		}
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		match b {
			18u8 => Ok(Self::Standard),
			39u8 => Ok(Self::Hardened),
			56u8 => Ok(Self::Paranoid),
			_ => Err(Error::Validity),
		}
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = [0u8; Self::OUTPUT_LEN];
		reader.read_exact(&mut b)?;
		Self::from_bytes(b[0])
	}
}

impl HeaderEncode for HashingAlgorithm {
	const OUTPUT_LEN: usize = 1 + Params::OUTPUT_LEN;
	type Identifier = [u8; 2];
	type Output = [u8; Self::OUTPUT_LEN];

	fn as_bytes(&self) -> Self::Output {
		match self {
			Self::Argon2id(p) => [0xF2u8, p.as_bytes()],
			Self::Blake3Balloon(p) => [0xA8u8, p.as_bytes()],
		}
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		let x = match b[0] {
			0xF2u8 => Self::Argon2id(Params::from_bytes(b[1])?),
			0xA8u8 => Self::Blake3Balloon(Params::from_bytes(b[1])?),
			_ => return Err(Error::Validity),
		};

		Ok(x)
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = Self::Output::default();
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl HeaderEncode for Algorithm {
	const OUTPUT_LEN: usize = 2;
	type Identifier = [u8; 2];
	type Output = [u8; Self::OUTPUT_LEN];

	fn as_bytes(&self) -> Self::Output {
		let s = match self {
			Self::Aes256GcmSiv => 0xD3,
			Self::XChaCha20Poly1305 => 0xD5,
		};

		[13u8, s]
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[0] != 13u8 {
			return Err(Error::Validity);
		}

		let a = match b[1] {
			0xD3 => Self::Aes256GcmSiv,
			0xD5 => Self::XChaCha20Poly1305,
			_ => return Err(Error::Validity),
		};

		Ok(a)
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = Self::Output::default();
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl HeaderEncode for Salt {
	const OUTPUT_LEN: usize = SALT_LEN + 2;
	type Identifier = [u8; 2];
	type Output = [u8; 18];

	fn as_bytes(&self) -> Self::Output {
		let mut s = [0u8; Self::OUTPUT_LEN];
		s[0] = 12u8;
		s[1] = 4u8;
		s[2..].copy_from_slice(self.inner());
		s
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[..2] != [12u8, 4u8] {
			return Err(Error::Validity);
		}

		let mut o = [0u8; SALT_LEN];
		o.copy_from_slice(&b[2..]);

		Ok(Self::new(o))
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = Self::Output::default();
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl HeaderEncode for Nonce {
	const OUTPUT_LEN: usize = 32;
	type Identifier = [u8; 2];
	type Output = [u8; Self::OUTPUT_LEN];

	fn as_bytes(&self) -> Self::Output {
		let b = match self {
			Self::Aes256GcmSiv(_) => 0xB5u8,
			Self::XChaCha20Poly1305(_) => 0xB7u8,
		};

		let len = self.algorithm().nonce_len();

		let mut s = [0u8; Self::OUTPUT_LEN];
		s[0] = 99u8;
		s[1] = b;
		s[2..len + 2].copy_from_slice(self.inner());

		s[len + 2..].copy_from_slice(&self.inner()[..Self::OUTPUT_LEN - 2 - len]);

		s
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[0] != 99u8 {
			return Err(Error::Validity);
		}

		let x = match b[1] {
			0xB5u8 => Self::Aes256GcmSiv(b[2..2 + AES_256_GCM_SIV_NONCE_LEN].to_array()?),
			0xB7u8 => Self::XChaCha20Poly1305(b[2..2 + XCHACHA20_POLY1305_NONCE_LEN].to_array()?),
			_ => return Err(Error::Validity),
		};

		Ok(x)
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = Self::Output::default();
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl HeaderEncode for EncryptedKey {
	const OUTPUT_LEN: usize = ENCRYPTED_KEY_LEN + Nonce::OUTPUT_LEN + 2;
	type Identifier = [u8; 2];
	type Output = Vec<u8>;

	fn as_bytes(&self) -> Self::Output {
		let mut s = Vec::with_capacity(Self::OUTPUT_LEN);

		s.extend_from_slice(&[0x9, 0xF3]);
		s.extend_from_slice(self.inner());
		s.extend_from_slice(&self.nonce().as_bytes());
		s
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[..2] != [9u8, 0xF3u8] {
			return Err(Error::Validity);
		}

		let e = Vec::from(&b[2..ENCRYPTED_KEY_LEN]).to_array()?;
		let n = Nonce::from_bytes(b[2 + ENCRYPTED_KEY_LEN..].to_array()?)?;

		Ok(Self::new(e, n))
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = vec![0u8; Self::OUTPUT_LEN];
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl HeaderEncode for Keyslot {
	const OUTPUT_LEN: usize =
		EncryptedKey::OUTPUT_LEN + (Salt::OUTPUT_LEN * 2) + HashingAlgorithm::OUTPUT_LEN + 2;
	type Identifier = [u8; 2];
	type Output = Vec<u8>;

	fn as_bytes(&self) -> Self::Output {
		let mut o = vec![0x83, 0x31];
		o.extend_from_slice(&self.hashing_algorithm.as_bytes());
		o.extend_from_slice(&self.hash_salt.as_bytes());
		o.extend_from_slice(&self.salt.as_bytes());
		o.extend_from_slice(&self.encrypted_key.as_bytes());
		o
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[..2] != [0x83, 0x21] {
			return Err(Error::Validity);
		}

		let hashing_algorithm = HashingAlgorithm::from_bytes(b[2..4].to_array()?)?;
		let hash_salt = Salt::from_bytes(b[4..Salt::OUTPUT_LEN + 4].to_array()?)?;
		let salt = Salt::from_bytes(b[Salt::OUTPUT_LEN + 8..Salt::OUTPUT_LEN + 12].to_array()?)?;
		let ek = EncryptedKey::from_bytes(b[Salt::OUTPUT_LEN + 12..].to_vec())?;

		Ok(Self {
			hashing_algorithm,
			hash_salt,
			salt,
			encrypted_key: ek,
		})
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = vec![0u8; Self::OUTPUT_LEN];
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl HeaderEncode for HeaderObject {
	const OUTPUT_LEN: usize = 0;
	type Identifier = [u8; 2];
	type Output = Vec<u8>;

	fn as_bytes(&self) -> Self::Output {
		let mut o = Vec::new();

		o.extend_from_slice(&[0xF1, 51u8]);
		o.extend_from_slice(&self.identifier.as_bytes());
		o.extend_from_slice(&self.nonce.as_bytes());

		// SAFETY: this unwrap is safe as the length of the objects is capped
		// will be removed in a trait overhaul which focuses on versioning too
		#[allow(clippy::unwrap_used)]
		o.extend_from_slice(&(TryInto::<u64>::try_into(self.data.len()).unwrap()).to_le_bytes());
		o.extend_from_slice(&self.data);

		o
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[..2] != [0xF1, 51u8] {
			return Err(Error::Validity);
		}

		let identifier =
			HeaderObjectIdentifier::from_bytes(b[2..HeaderObjectIdentifier::OUTPUT_LEN].to_vec())?;
		let nonce = Nonce::from_bytes(
			b[HeaderObjectIdentifier::OUTPUT_LEN + 2
				..HeaderObjectIdentifier::OUTPUT_LEN + 2 + Nonce::OUTPUT_LEN]
				.to_array()?,
		)?;
		let data_len = u64::from_le_bytes(
			b[HeaderObjectIdentifier::OUTPUT_LEN + Nonce::OUTPUT_LEN + 2
				..HeaderObjectIdentifier::OUTPUT_LEN + Nonce::OUTPUT_LEN + 2 + 8]
				.to_array()?,
		);
		let data = b[HeaderObjectIdentifier::OUTPUT_LEN + Nonce::OUTPUT_LEN + 10
			..data_len.try_into().map_err(|_| Error::Validity)?]
			.to_vec();

		Ok(Self {
			identifier,
			nonce,
			data,
		})
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut buffer = [0u8; mem::size_of::<u64>()];
		reader.read_exact(&mut buffer)?;
		let size = u64::from_le_bytes(buffer);

		let mut buffer = vec![0u8; size.try_into().map_err(|_| Error::Validity)?];
		reader.read_exact(&mut buffer)?;

		Self::from_bytes(buffer)
	}
}

impl HeaderEncode for HeaderObjectIdentifier {
	const OUTPUT_LEN: usize = 2 + EncryptedKey::OUTPUT_LEN + Salt::OUTPUT_LEN;
	type Identifier = [u8; 2];
	type Output = Vec<u8>;

	fn as_bytes(&self) -> Self::Output {
		let mut o = vec![0xC2, 0xE9];
		o.extend_from_slice(&self.key.as_bytes());
		o.extend_from_slice(&self.salt.as_bytes());
		o
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[..2] != [0xC2, 0xE9] {
			return Err(Error::Validity);
		}

		let ek = EncryptedKey::from_bytes(b[2..EncryptedKey::OUTPUT_LEN].to_vec())?;
		let salt = Salt::from_bytes(b[EncryptedKey::OUTPUT_LEN + 2..].to_array()?)?;

		Ok(Self { key: ek, salt })
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = vec![0u8; Self::OUTPUT_LEN];
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl HeaderEncode for HeaderVersion {
	const OUTPUT_LEN: usize = 2;
	type Identifier = [u8; 2];
	type Output = [u8; Self::OUTPUT_LEN];

	fn as_bytes(&self) -> Self::Output {
		match self {
			Self::V1 => [0xDA; 2],
		}
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		match b {
			[0xDA, 0xDA] => Ok(Self::V1),
			_ => Err(Error::Validity),
		}
	}

	fn from_reader<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut b = [0u8; Self::OUTPUT_LEN];
		reader.read_exact(&mut b)?;
		Self::from_bytes(b)
	}
}

impl Header {
	pub fn as_bytes(&self) -> Result<Vec<u8>> {
		match self.version {
			HeaderVersion::V1 => self.as_bytes_v1(),
		}
	}

	fn as_bytes_v1(&self) -> Result<Vec<u8>> {
		let mut o = vec![];
		o.extend_from_slice(&[0xFA, 0xDA]);

		o.extend_from_slice(&self.version.as_bytes());
		o.extend_from_slice(&self.algorithm.as_bytes());
		o.extend_from_slice(&self.nonce.as_bytes());

		self.keyslots
			.iter()
			.for_each(|k| o.extend_from_slice(&k.as_bytes()));

		(0..KEYSLOT_LIMIT - self.keyslots.len())
			.for_each(|_| o.extend_from_slice(&Keyslot::random().as_bytes()));

		o.extend_from_slice(
			&(TryInto::<u16>::try_into(self.objects.len()).map_err(|_| Error::Validity)?)
				.to_le_bytes(),
		);

		self.objects.iter().try_for_each(|k| {
			let b = k.as_bytes();
			o.extend_from_slice(
				&(TryInto::<u64>::try_into(b.len()).map_err(|_| Error::Validity)?).to_le_bytes(),
			);
			o.extend_from_slice(&b);

			Ok::<_, Error>(())
		})?;

		Ok(o)
	}

	pub(super) fn from_reader_raw<R>(reader: &mut R) -> Result<Self>
	where
		R: std::io::Read + std::io::Seek,
	{
		let mut m = [0u8; 2];
		reader.read_exact(&mut m)?;

		if m != [0xFA, 0xDA] {
			return Err(Error::Validity);
		}

		let mut buffer = [0u8; HeaderVersion::OUTPUT_LEN];
		reader.read_exact(&mut buffer)?;
		let version = HeaderVersion::from_bytes(buffer)?;

		let mut buffer = [0u8; Algorithm::OUTPUT_LEN];
		reader.read_exact(&mut buffer)?;
		let algorithm = Algorithm::from_bytes(buffer)?;

		let mut nonce_buffer = [0u8; Nonce::OUTPUT_LEN];
		reader.read_exact(&mut nonce_buffer)?;
		let nonce = Nonce::from_bytes(nonce_buffer)?;
		nonce.validate(algorithm)?;

		// we always read the limit as there will always be extra room for additional keyslots after header creation
		let keyslots = (0..KEYSLOT_LIMIT)
			.filter_map(|_| {
				let mut buffer = [0u8; Keyslot::OUTPUT_LEN];
				reader.read_exact(&mut buffer).ok();
				Keyslot::from_bytes(buffer.to_vec()).ok()
			})
			.collect::<Vec<Keyslot>>();

		let mut buffer = [0u8; mem::size_of::<u16>()];
		reader.read_exact(&mut buffer)?;
		let objects_len = u16::from_le_bytes(buffer);

		let objects = (0..objects_len)
			.map(|_| HeaderObject::from_reader(reader))
			.collect::<Result<Vec<HeaderObject>>>()?;

		let h = Self {
			version,
			algorithm,
			nonce,
			keyslots,
			objects,
		};

		Ok(h)
	}
}
