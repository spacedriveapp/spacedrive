use zeroize::Zeroize;
mod chacha20;
pub use chacha20::CryptoRng;

impl rand::CryptoRng for CryptoRng {}

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
