//! Stand-in for the `rc4` crate.
//!
//! `smb-server` reaches for RC4 in exactly one place — unwrapping the
//! encrypted session key during NTLM authentication — and the published
//! `rc4 0.2` pulls `cipher 0.5`, which pulls `crypto-common 0.2.2`. That
//! collides with the `crypto-common 0.2.0-rc.4` that `iroh` pins through its
//! exact `aead = "=0.6.0-rc.2"` requirement, and cargo can only choose one
//! version inside a semver-compatible range.
//!
//! Rather than fork twelve thousand lines of SMB server over a three-line
//! call site, this crate provides the same surface with no dependencies at
//! all. It is a workaround for a pin we do not control: once iroh moves off
//! the aead pre-release, delete this crate and the `[patch]` entry and take
//! the real one.
//!
//! RC4 is broken as a cipher. It exists here because NTLM specifies it; it
//! is never used for anything Spacedrive protects.

#![forbid(unsafe_code)]

/// The traits `smb-server` imports from `rc4::cipher`. Same names and same
/// shapes as the RustCrypto ones for the calls that are actually made.
pub mod cipher {
	/// Construction from raw key bytes.
	pub trait KeyInit: Sized {
		fn new_from_slice(key: &[u8]) -> Result<Self, InvalidLength>;
	}

	/// In-place keystream application.
	pub trait StreamCipher {
		fn apply_keystream(&mut self, data: &mut [u8]);
	}

	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub struct InvalidLength;

	impl core::fmt::Display for InvalidLength {
		fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
			f.write_str("invalid key length")
		}
	}

	impl std::error::Error for InvalidLength {}
}

use cipher::{InvalidLength, KeyInit, StreamCipher};

/// RC4 state: the permutation plus the two stream indices.
pub struct Rc4 {
	state: [u8; 256],
	i: u8,
	j: u8,
}

impl KeyInit for Rc4 {
	fn new_from_slice(key: &[u8]) -> Result<Self, InvalidLength> {
		if key.is_empty() || key.len() > 256 {
			return Err(InvalidLength);
		}
		let mut state = [0u8; 256];
		for (i, slot) in state.iter_mut().enumerate() {
			*slot = i as u8;
		}
		// Key-scheduling algorithm.
		let mut j = 0u8;
		for i in 0..256usize {
			j = j.wrapping_add(state[i]).wrapping_add(key[i % key.len()]);
			state.swap(i, j as usize);
		}
		Ok(Self { state, i: 0, j: 0 })
	}
}

impl StreamCipher for Rc4 {
	fn apply_keystream(&mut self, data: &mut [u8]) {
		// Pseudo-random generation algorithm, XORed in place.
		for byte in data.iter_mut() {
			self.i = self.i.wrapping_add(1);
			self.j = self.j.wrapping_add(self.state[self.i as usize]);
			self.state.swap(self.i as usize, self.j as usize);
			let k = self.state[self.i as usize].wrapping_add(self.state[self.j as usize]);
			*byte ^= self.state[k as usize];
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Test vectors from RFC 6229, section 2.
	#[test]
	fn rfc6229_vectors() {
		let mut rc4 = Rc4::new_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05]).unwrap();
		let mut stream = [0u8; 16];
		rc4.apply_keystream(&mut stream);
		assert_eq!(
			stream,
			[
				0xb2, 0x39, 0x63, 0x05, 0xf0, 0x3d, 0xc0, 0x27, 0xcc, 0xc3, 0x52, 0x4a, 0x0a, 0x11,
				0x18, 0xa8
			]
		);

		let mut rc4 = Rc4::new_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]).unwrap();
		let mut stream = [0u8; 16];
		rc4.apply_keystream(&mut stream);
		assert_eq!(
			stream,
			[
				0x29, 0x3f, 0x02, 0xd4, 0x7f, 0x37, 0xc9, 0xb6, 0x33, 0xf2, 0xaf, 0x52, 0x85, 0xfe,
				0xb4, 0x6b
			]
		);
	}

	#[test]
	fn round_trips() {
		let key = b"spacedrive";
		let plain = b"the quick brown fox";
		let mut buf = *plain;
		Rc4::new_from_slice(key).unwrap().apply_keystream(&mut buf);
		assert_ne!(&buf, plain);
		Rc4::new_from_slice(key).unwrap().apply_keystream(&mut buf);
		assert_eq!(&buf, plain);
	}

	#[test]
	fn rejects_empty_key() {
		assert!(Rc4::new_from_slice(&[]).is_err());
	}
}
