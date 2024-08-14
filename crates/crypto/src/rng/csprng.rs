use rand::RngCore;
use rand_chacha::ChaCha20Rng;
use rand_core::{impl_try_crypto_rng_from_crypto_rng, SeedableRng};
use zeroize::{Zeroize, Zeroizing};

/// This RNG should be used throughout the entire crate.
///
/// On `Drop`, it re-seeds the inner RNG, erasing the previous state and making all future
/// values unpredictable.
#[derive(Debug)]
pub struct CryptoRng(ChaCha20Rng);

impl CryptoRng {
	/// This creates a new [`ChaCha20Rng`]-backed [`rand::CryptoRng`] from entropy
	/// (via the [getrandom](https://docs.rs/getrandom) crate).
	#[inline]
	#[must_use]
	pub fn new() -> Self {
		Self(ChaCha20Rng::from_os_rng())
	}

	/// Used to generate completely random bytes, with the use of [`ChaCha20Rng`]
	///
	/// Ideally this should be used for small amounts only (as it's stack allocated)
	#[inline]
	#[must_use]
	pub fn generate_fixed<const I: usize>(&mut self) -> [u8; I] {
		let mut bytes = Zeroizing::new([0u8; I]);
		self.fill_bytes(bytes.as_mut());
		*bytes
	}

	/// Used to generate completely random bytes, with the use of [`ChaCha20Rng`]
	#[inline]
	#[must_use]
	pub fn generate_vec(&mut self, size: usize) -> Vec<u8> {
		let mut bytes = vec![0u8; size];
		self.fill_bytes(bytes.as_mut());
		bytes
	}
}

impl RngCore for CryptoRng {
	#[inline]
	fn fill_bytes(&mut self, dest: &mut [u8]) {
		self.0.fill_bytes(dest);
	}

	#[inline]
	fn next_u32(&mut self) -> u32 {
		self.0.next_u32()
	}

	#[inline]
	fn next_u64(&mut self) -> u64 {
		self.0.next_u64()
	}
}

impl Zeroize for CryptoRng {
	#[inline]
	fn zeroize(&mut self) {
		self.0 = ChaCha20Rng::from_os_rng();
	}
}

impl rand::CryptoRng for CryptoRng {}

impl_try_crypto_rng_from_crypto_rng!(CryptoRng);

impl Default for CryptoRng {
	fn default() -> Self {
		Self::new()
	}
}

impl Drop for CryptoRng {
	#[inline]
	fn drop(&mut self) {
		self.zeroize();
	}
}
