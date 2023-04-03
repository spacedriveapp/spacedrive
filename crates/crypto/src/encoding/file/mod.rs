use crate::{
	primitives::{
		AES_256_GCM_NONCE_LEN, AES_256_GCM_SIV_NONCE_LEN, ENCRYPTED_KEY_LEN, SALT_LEN,
		XCHACHA20_POLY1305_NONCE_LEN,
	},
	types::{Algorithm, EncryptedKey, HashingAlgorithm, Nonce, Params, Salt},
	utils::{generate_fixed, ToArray},
	Error, Result,
};

use self::header::HeaderVersion;

pub mod header;
pub mod keyslot;
pub mod object;

// pub use all the types?
const KEYSLOT_LIMIT: usize = 2;
const OBJECT_LIMIT: usize = 2;

pub trait HeaderEncode {
	type Output;

	fn as_bytes(&self) -> Self::Output;
	fn from_bytes(b: Self::Output) -> Result<Self>
	where
		Self: Sized;
}

impl HeaderEncode for Params {
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
}

impl HeaderEncode for HashingAlgorithm {
	type Output = [u8; 2];
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
}

impl HeaderEncode for Algorithm {
	type Output = [u8; 2];

	fn as_bytes(&self) -> Self::Output {
		let s = match self {
			Self::Aes256Gcm => 0xD1,
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
			0xD1 => Self::Aes256Gcm,
			0xD3 => Self::Aes256GcmSiv,
			0xD5 => Self::XChaCha20Poly1305,
			_ => return Err(Error::Validity),
		};

		Ok(a)
	}
}

impl HeaderEncode for Salt {
	type Output = [u8; SALT_LEN + 2];
	fn as_bytes(&self) -> Self::Output {
		let mut s = [0u8; SALT_LEN + 2];
		s[0] = 12u8;
		s[1] = 4u8;
		s[2..].copy_from_slice(self.inner());
		s
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[..2] != [12u8, 4u8] {
			return Err(Error::Validity);
		}

		let mut o = [0u8; 16];
		o.copy_from_slice(&b[2..]);

		Ok(Self::new(o))
	}
}

impl HeaderEncode for Nonce {
	type Output = [u8; 32];

	fn as_bytes(&self) -> Self::Output {
		let b = match self {
			Self::Aes256Gcm(_) => 0xB2u8,
			Self::Aes256GcmSiv(_) => 0xB5u8,
			Self::XChaCha20Poly1305(_) => 0xB7u8,
		};

		let mut s: [u8; 32] = generate_fixed();
		s[0] = 99u8;
		s[1] = b;
		s[2..].copy_from_slice(self.inner());
		s
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[0] != 99u8 {
			return Err(Error::Validity);
		}

		let x = match b[1] {
			0xB2u8 => Self::Aes256Gcm(b[2..AES_256_GCM_NONCE_LEN].to_array()?),
			0xB5u8 => Self::Aes256GcmSiv(b[2..AES_256_GCM_SIV_NONCE_LEN].to_array()?),
			0xB7u8 => Self::XChaCha20Poly1305(b[2..XCHACHA20_POLY1305_NONCE_LEN].to_array()?),
			_ => return Err(Error::Validity),
		};

		Ok(x)
	}
}

impl HeaderEncode for HeaderVersion {
	type Output = [u8; 2];

	fn as_bytes(&self) -> Self::Output {
		let b = match self {
			Self::V1 => 0xE1,
		};

		[0xEE, b]
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[0] != 0xEE {
			return Err(Error::Validity);
		}

		let x = match b[1] {
			0xE1 => Self::V1,
			_ => return Err(Error::Validity),
		};

		Ok(x)
	}
}

impl HeaderEncode for EncryptedKey {
	type Output = [u8; ENCRYPTED_KEY_LEN + 32 + 2];
	fn as_bytes(&self) -> Self::Output {
		let mut s = [0u8; ENCRYPTED_KEY_LEN + 32 + 2];
		s[0] = 9u8;
		s[1] = 0xF3u8;
		s[2..].copy_from_slice(self.inner());
		s[2 + ENCRYPTED_KEY_LEN..].copy_from_slice(&self.nonce().as_bytes());
		s
	}

	fn from_bytes(b: Self::Output) -> Result<Self> {
		if b[..2] != [9u8, 0xF3u8] {
			return Err(Error::Validity);
		}

		let mut e = [0u8; ENCRYPTED_KEY_LEN];
		e.copy_from_slice(&b[2..ENCRYPTED_KEY_LEN]);
		let n = Nonce::from_bytes(b[2 + ENCRYPTED_KEY_LEN..].to_array()?)?;

		Ok(Self::new(e, n))
	}
}
