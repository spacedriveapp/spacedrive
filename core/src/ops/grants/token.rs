//! Grant bearer tokens.
//!
//! A token is `sdg_` followed by 64 hex characters, 32 bytes from the
//! operating system's random source. The prefix lets a secret scanner or a
//! person recognize a leaked token at a glance. Only the SHA-256 of the whole
//! string is stored, so a lookup is by hash and the token never touches disk.

use rand::RngCore;
use sha2::{Digest, Sha256};

/// The prefix every grant token carries.
pub const TOKEN_PREFIX: &str = "sdg_";

const SECRET_BYTES: usize = 32;

/// A fresh token and the hash to store for it.
pub fn mint() -> (String, String) {
	let mut secret = [0u8; SECRET_BYTES];
	rand::rngs::OsRng.fill_bytes(&mut secret);
	let token = format!("{TOKEN_PREFIX}{}", hex::encode(secret));
	let hash = digest(&token);
	(token, hash)
}

/// The stored hash for `token`, or `None` when it cannot be a grant token.
///
/// Rejecting malformed input before hashing keeps arbitrary header values
/// from reaching the database.
pub fn hash(token: &str) -> Option<String> {
	let secret = token.strip_prefix(TOKEN_PREFIX)?;
	if secret.len() != SECRET_BYTES * 2 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
		return None;
	}
	Some(digest(token))
}

fn digest(token: &str) -> String {
	hex::encode(Sha256::digest(token.as_bytes()))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_minted_token_hashes_to_its_stored_hash() {
		let (token, stored) = mint();
		assert!(token.starts_with(TOKEN_PREFIX));
		assert_eq!(token.len(), TOKEN_PREFIX.len() + 64);
		assert_eq!(hash(&token).as_deref(), Some(stored.as_str()));
		assert_ne!(stored, token, "the stored value is not the token");
	}

	#[test]
	fn two_tokens_differ() {
		assert_ne!(mint().0, mint().0);
	}

	#[test]
	fn malformed_tokens_have_no_hash() {
		let (token, _) = mint();
		assert_eq!(hash(&token[TOKEN_PREFIX.len()..]), None, "missing prefix");
		assert_eq!(hash(&token[..token.len() - 1]), None, "short");
		assert_eq!(hash(&format!("{token}0")), None, "long");
		assert_eq!(hash(&format!("{TOKEN_PREFIX}{}", "z".repeat(64))), None);
		assert_eq!(hash(""), None);
	}
}
