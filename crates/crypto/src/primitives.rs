// This is the default salt size, and the recommended size for argon2id.
pub const SALT_LEN: usize = 16;

/// The size used for streaming blocks. This size seems to offer the best performance compared to alternatives.
/// The file size gain is 16 bytes per 1MiB (due to the AEAD tag)
pub const BLOCK_SIZE: usize = 1048576;

// These are all possible algorithms that can be used for encryption
// They tie in heavily with `StreamEncryption` and `StreamDecryption`
#[derive(Clone, Copy, PartialEq)]
pub enum Algorithm {
	XChaCha20Poly1305,
	Aes256Gcm,
}

// These are the different "modes" for encryption
// Stream works in "blocks", incrementing the nonce on each block (so the same nonce isn't used twice)
// Memory loads all data into memory before encryption, and encrypts it in one pass.
// Stream mode is going to be the default for files, containers, etc. as  memory usage is roughly equal to the `BLOCK_SIZE`
// Memory mode is only going to be used for small amounts of data (such as a master key) - streaming modes aren't viable here
#[derive(PartialEq)]
pub enum Mode {
	Stream,
	Memory,
}

impl Algorithm {
	// This function calculates the expected nonce length for a given algorithm
	// 4 bytes are deducted for streaming mode, due to the LE31 counter being the last 4 bytes of the nonce
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
