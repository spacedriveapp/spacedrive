/// Basic Functions to encrypt and decrypt data
/// Feel free to add functions based on standards or your own implementation as we use more than one encryption method.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce
};
use rand::Rng;

pub fn encrypt_string(plaintext: &str, key_str: &str) -> Result<Vec<u8>, String> {
    // Create key using TryFrom
    let key_hash = blake3::hash(key_str.as_bytes());
    let key = Key::try_from(&key_hash.as_bytes()[..32])
        .map_err(|e| e.to_string())?;

    // Initialize cipher
    let cipher = ChaCha20Poly1305::new(&key);

    // Create random nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill(&mut nonce_bytes);
    let nonce = Nonce::try_from(nonce_bytes.as_slice())
        .map_err(|e| e.to_string())?;

    // Encrypt
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|e| e.to_string())?;

    // Combine nonce and ciphertext
    let mut final_bytes = nonce_bytes.to_vec();
    final_bytes.extend(ciphertext);

    Ok(final_bytes)
}

pub fn decrypt_string(encrypted: &[u8], key_str: &str) -> Result<String, String> {
	// Create key using TryFrom
	let key_hash = blake3::hash(key_str.as_bytes());
	let key = Key::try_from(&key_hash.as_bytes()[..32])
		.map_err(|e| e.to_string())?;

	// Split nonce and ciphertext
	let (nonce_bytes, ciphertext) = encrypted.split_at(12);
	let nonce = Nonce::try_from(nonce_bytes)
		.map_err(|e| e.to_string())?;

	// Initialize cipher
	let cipher = ChaCha20Poly1305::new(&key);

	// Decrypt
	let plaintext = cipher
		.decrypt(&nonce, ciphertext)
		.map_err(|e| e.to_string())?;

    String::from_utf8(plaintext).map_err(|e| e.to_string())
}