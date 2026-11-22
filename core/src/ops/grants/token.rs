//! Grant bearer tokens and OAuth refresh tokens.
//!
//! A token is `sdg_` followed by 64 hex characters, 32 bytes from the
//! operating system's random source. The prefix lets a secret scanner or a
//! person recognize a leaked token at a glance. Only the SHA-256 of the whole
//! string is stored, so a lookup is by hash and the token never touches disk.
//!
//! A grant minted through OAuth also has a refresh token, `sdr_` and 64 hex
//! characters, stored the same way. The distinct prefix keeps a refresh token
//! from ever being accepted as a bearer token.

use rand::RngCore;
use sha2::{Digest, Sha256};

/// The prefix every grant token carries.
pub const TOKEN_PREFIX: &str = "sdg_";
/// The prefix every OAuth refresh token carries.
pub const REFRESH_PREFIX: &str = "sdr_";

const SECRET_BYTES: usize = 32;

/// A fresh token and the hash to store for it.
pub fn mint() -> (String, String) {
	mint_with(TOKEN_PREFIX)
}

/// A fresh refresh token and the hash to store for it.
pub fn mint_refresh() -> (String, String) {
	mint_with(REFRESH_PREFIX)
}

/// The stored hash for `token`, or `None` when it cannot be a grant token.
///
/// Rejecting malformed input before hashing keeps arbitrary header values
/// from reaching the database.
pub fn hash(token: &str) -> Option<String> {
	hash_with(TOKEN_PREFIX, token)
}

/// The stored hash for a refresh token, or `None` when it cannot be one.
pub fn hash_refresh(token: &str) -> Option<String> {
	hash_with(REFRESH_PREFIX, token)
}

fn mint_with(prefix: &str) -> (String, String) {
	let mut secret = [0u8; SECRET_BYTES];
	rand::rngs::OsRng.fill_bytes(&mut secret);
	let token = format!("{prefix}{}", hex::encode(secret));
	let hash = digest(&token);
	(token, hash)
}

fn hash_with(prefix: &str, token: &str) -> Option<String> {
	let secret = token.strip_prefix(prefix)?;
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
	fn refresh_and_grant_tokens_are_not_interchangeable() {
		let (refresh, stored) = mint_refresh();
		assert!(refresh.starts_with(REFRESH_PREFIX));
		assert_eq!(hash_refresh(&refresh).as_deref(), Some(stored.as_str()));
		assert_eq!(hash(&refresh), None);
		assert_eq!(hash_refresh(&mint().0), None);
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
