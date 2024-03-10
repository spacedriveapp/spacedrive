use rand::RngCore;
use rand_chacha::ChaCha20Rng;
use rand_core::{block::BlockRngCore, SeedableRng};
use zeroize::{Zeroize, Zeroizing};

const STATE_WORDS: usize = 16;

/// This RNG should be used throughout the entire crate.
///
/// On `Drop`, it re-seeds the inner RNG, erasing the previous state and making all future
/// values unpredictable.
pub struct CryptoRng(Box<ChaCha20Rng>);

impl CryptoRng {
	/// This creates a new `ChaCha20Rng`-backed `CryptoRng` from entropy (via the `getrandom` crate).
	#[inline]
	#[must_use]
	pub fn new() -> Self {
		Self(Box::new(ChaCha20Rng::from_entropy()))
	}

	/// Used to generate completely random bytes, with the use of `ChaCha20`
	///
	/// Ideally this should be used for small amounts only (as it's stack allocated)
	#[inline]
	#[must_use]
	pub fn generate_fixed<const I: usize>() -> [u8; I] {
		let mut bytes = Zeroizing::new([0u8; I]);
		Self::new().0.fill_bytes(bytes.as_mut());
		*bytes
	}

	/// Used to generate completely random bytes, with the use of `ChaCha20`
	#[inline]
	#[must_use]
	pub fn generate_vec(size: usize) -> Vec<u8> {
		let mut bytes = Zeroizing::new(vec![0u8; size]);
		Self::new().fill_bytes(bytes.as_mut());
		bytes.to_vec()
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

	#[inline]
	fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
		self.0.try_fill_bytes(dest)
	}
}

impl BlockRngCore for CryptoRng {
	type Item = u32;
	type Results = [u32; STATE_WORDS];

	#[inline]
	fn generate(&mut self, results: &mut Self::Results) {
		(0..STATE_WORDS).for_each(|i| results[i] = self.next_u32());
	}
}

impl Zeroize for CryptoRng {
	#[inline]
	fn zeroize(&mut self) {
		*self.0 = ChaCha20Rng::from_entropy();
	}
}
