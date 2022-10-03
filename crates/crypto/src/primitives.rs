pub const SALT_LEN: usize = 16;

/// The size used for STREAM blocks. This size seems to offer the best performance compared to alternatives.
/// The file size gain is 16 bytes per 1MiB.
pub const BLOCK_SIZE: usize = 1048576;

#[derive(Clone, Copy, PartialEq)]
pub enum Algorithm {
	XChaCha20Poly1305,
	Aes256Gcm,
}

#[derive(PartialEq)]
pub enum Mode {
	Stream,
	Memory,
}

impl Algorithm {
	/// This function calculates the expected nonce length for a given algorithm
	pub fn nonce_len(&self, mode: Mode) -> usize {
		let base = match self {
			Self::XChaCha20Poly1305 => 24,
			Self::Aes256Gcm => 12,
		};

		if mode == Mode::Stream {
			base - 4
		} else {
			base
		}
	}
}