//! Signed grant tokens.
//!
//! An opaque `sdg_` token means something only to the device that stored its
//! hash. A signed token carries its own claims, so any node that holds the
//! space's store can check it: the issuing device's public key is published
//! there as an assertion, and revocations reach the node the same way.
//!
//! A token is `sdg_s.`, the claims as JSON in unpadded base64url, a dot, and an
//! Ed25519 signature over a domain label followed by those exact bytes, also
//! base64url. The signature covers the bytes as sent rather than a
//! re-serialization, so no two encoders can disagree about what was signed.
//! The label keeps a grant signature from ever being mistaken for any other
//! signature the same key makes. The prefix starts with `sdg_`, so every log
//! filter, URL check and secret scanner that matches opaque tokens matches
//! signed ones too, while [`super::token::hash`] never accepts one.
//!
//! The signing key is derived from the device's master key with its own HKDF
//! label, so it is stable across restarts, never stored, and distinct from the
//! network identity key derived from the same master key.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use super::{GrantCap, GrantError};

/// The prefix every signed grant token carries.
pub const SIGNED_PREFIX: &str = "sdg_s.";

/// The claims format this build writes and reads.
pub const CLAIMS_VERSION: u8 = 1;

const SIGNATURE_DOMAIN: &[u8] = b"spacedrive grant token v1\0";
const KEY_LABEL: &[u8] = b"spacedrive-grant-signing-v1";

/// Longer than any token this build mints. Refusing longer input before
/// decoding keeps an oversized header from costing anything.
const MAX_TOKEN_LEN: usize = 4096;

/// What a signed token says about itself. Trust none of it until
/// [`verify`] has checked the signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Claims {
	pub v: u8,
	pub grant_id: Uuid,
	pub space_id: Uuid,
	/// The granted folder, `/`-separated and relative to the space root.
	pub subtree: String,
	pub caps: GrantCap,
	/// Unix seconds after which the token is refused.
	pub exp: i64,
	/// The space's authority epoch the grant was minted under.
	pub epoch: i64,
	/// The device that signed the token.
	pub iss: Uuid,
}

impl Claims {
	pub fn expires_at(&self) -> DateTime<Utc> {
		DateTime::from_timestamp(self.exp, 0).unwrap_or(DateTime::<Utc>::MIN_UTC)
	}
}

/// The key this device signs grant tokens with, derived from its master key.
pub fn signing_key(master_key: &[u8; 32]) -> SigningKey {
	let mut seed = [0u8; 32];
	Hkdf::<Sha256>::new(None, master_key)
		.expand(KEY_LABEL, &mut seed)
		.expect("32 bytes is a valid HKDF-SHA256 output length");
	SigningKey::from_bytes(&seed)
}

/// Whether `token` is in the signed format, without checking anything else.
pub fn is_signed(token: &str) -> bool {
	token.starts_with(SIGNED_PREFIX)
}

/// Sign `claims` into a token, or `None` when the claims are too large for
/// [`verify`] to ever accept, which only a very long subtree can cause.
pub fn sign(key: &SigningKey, claims: &Claims) -> Option<String> {
	let payload = serde_json::to_vec(claims).expect("claims always serialize");
	let signature = key.sign(&signed_message(&payload));
	let token = format!(
		"{SIGNED_PREFIX}{}.{}",
		URL_SAFE_NO_PAD.encode(&payload),
		URL_SAFE_NO_PAD.encode(signature.to_bytes())
	);
	(token.len() <= MAX_TOKEN_LEN).then_some(token)
}

/// The claims of a signed token, unchecked.
///
/// A verifier needs the space and the issuer to find the key to check the
/// signature with, and they are inside the token. Nothing read here may be
/// acted on before [`verify`] succeeds.
pub fn unverified_claims(token: &str) -> Result<Claims, GrantError> {
	let (payload, _) = split(token)?;
	parse(&payload)
}

/// Check a signed token against its issuer's key at `now`.
///
/// A token that is malformed, signed by another key, or altered in any byte
/// answers [`GrantError::UnknownToken`]: to its holder it is no credential at
/// all. A genuine token past its `exp` answers [`GrantError::Expired`].
pub fn verify(token: &str, key: &VerifyingKey, now: DateTime<Utc>) -> Result<Claims, GrantError> {
	let claims = verify_signature(token, key)?;
	if now.timestamp() >= claims.exp {
		return Err(GrantError::Expired);
	}
	Ok(claims)
}

/// Check a signed token's signature against its issuer's key, whatever its
/// expiry, so a verifier can record the refusal of an expired but genuine
/// token against its grant.
pub fn verify_signature(token: &str, key: &VerifyingKey) -> Result<Claims, GrantError> {
	let (payload, signature) = split(token)?;
	let signature: [u8; 64] = signature.try_into().map_err(|_| GrantError::UnknownToken)?;
	key.verify_strict(
		&signed_message(&payload),
		&Signature::from_bytes(&signature),
	)
	.map_err(|_| GrantError::UnknownToken)?;
	parse(&payload)
}

fn split(token: &str) -> Result<(Vec<u8>, Vec<u8>), GrantError> {
	if token.len() > MAX_TOKEN_LEN {
		return Err(GrantError::UnknownToken);
	}
	let body = token
		.strip_prefix(SIGNED_PREFIX)
		.ok_or(GrantError::UnknownToken)?;
	let (payload, signature) = body.split_once('.').ok_or(GrantError::UnknownToken)?;
	let decode = |part: &str| {
		URL_SAFE_NO_PAD
			.decode(part)
			.map_err(|_| GrantError::UnknownToken)
	};
	Ok((decode(payload)?, decode(signature)?))
}

fn parse(payload: &[u8]) -> Result<Claims, GrantError> {
	let claims: Claims = serde_json::from_slice(payload).map_err(|_| GrantError::UnknownToken)?;
	if claims.v != CLAIMS_VERSION {
		return Err(GrantError::UnknownToken);
	}
	Ok(claims)
}

fn signed_message(payload: &[u8]) -> Vec<u8> {
	[SIGNATURE_DOMAIN, payload].concat()
}

#[cfg(test)]
mod tests {
	use chrono::Duration;

	use super::*;

	fn key(byte: u8) -> SigningKey {
		signing_key(&[byte; 32])
	}

	fn claims(now: DateTime<Utc>) -> Claims {
		Claims {
			v: CLAIMS_VERSION,
			grant_id: Uuid::from_u128(1),
			space_id: Uuid::from_u128(2),
			subtree: "notes".into(),
			caps: GrantCap::Read,
			exp: (now + Duration::hours(1)).timestamp(),
			epoch: 3,
			iss: Uuid::from_u128(4),
		}
	}

	#[test]
	fn a_token_verifies_against_its_issuer_and_reads_back_its_claims() {
		let now = Utc::now();
		let key = key(1);
		let token = sign(&key, &claims(now)).unwrap();
		assert!(is_signed(&token));
		assert!(token.starts_with(super::super::token::TOKEN_PREFIX));
		assert_eq!(super::super::token::hash(&token), None);
		assert_eq!(
			verify(&token, &key.verifying_key(), now).unwrap(),
			claims(now)
		);
		assert_eq!(unverified_claims(&token).unwrap(), claims(now));
	}

	#[test]
	fn the_signing_key_is_stable_and_distinct_per_master_key() {
		assert_eq!(key(1).to_bytes(), key(1).to_bytes());
		assert_ne!(key(1).to_bytes(), key(2).to_bytes());
		assert_ne!(
			key(1).to_bytes(),
			[1u8; 32],
			"derived, never the master key itself"
		);
	}

	#[test]
	fn claims_too_large_to_verify_are_never_signed() {
		let now = Utc::now();
		let mut long = claims(now);
		long.subtree = vec!["folder"; 600].join("/");
		assert_eq!(sign(&key(1), &long), None);
	}

	#[test]
	fn an_expired_token_is_refused_as_expired() {
		let now = Utc::now();
		let key = key(1);
		let token = sign(&key, &claims(now)).unwrap();
		let at_expiry = claims(now).expires_at();
		assert_eq!(
			verify(&token, &key.verifying_key(), at_expiry),
			Err(GrantError::Expired)
		);
		assert_eq!(
			verify(&token, &key.verifying_key(), at_expiry + Duration::days(1)),
			Err(GrantError::Expired)
		);
		assert!(verify(
			&token,
			&key.verifying_key(),
			at_expiry - Duration::seconds(1)
		)
		.is_ok());
	}

	#[test]
	fn a_token_signed_by_another_key_is_refused() {
		let now = Utc::now();
		let token = sign(&key(1), &claims(now)).unwrap();
		assert_eq!(
			verify(&token, &key(2).verifying_key(), now),
			Err(GrantError::UnknownToken)
		);
	}

	#[test]
	fn any_altered_claim_breaks_the_signature() {
		let now = Utc::now();
		let key = key(1);
		let token = sign(&key, &claims(now)).unwrap();
		let signature = token.rsplit_once('.').unwrap().1;

		let mut widened = claims(now);
		widened.caps = GrantCap::ReadWrite;
		let mut moved = claims(now);
		moved.space_id = Uuid::from_u128(99);
		let mut rooted = claims(now);
		rooted.subtree = String::new();
		let mut longer = claims(now);
		longer.exp += 86_400;
		let mut fenced = claims(now);
		fenced.epoch += 1;
		for altered in [widened, moved, rooted, longer, fenced] {
			let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&altered).unwrap());
			let forged = format!("{SIGNED_PREFIX}{payload}.{signature}");
			assert_eq!(
				verify(&forged, &key.verifying_key(), now),
				Err(GrantError::UnknownToken),
				"{altered:?}"
			);
		}
	}

	#[test]
	fn a_flipped_byte_anywhere_is_refused() {
		let now = Utc::now();
		let key = key(1);
		let token = sign(&key, &claims(now)).unwrap();
		for index in SIGNED_PREFIX.len()..token.len() {
			let mut bytes = token.clone().into_bytes();
			bytes[index] = if bytes[index] == b'A' { b'B' } else { b'A' };
			let tampered = String::from_utf8(bytes).unwrap();
			if tampered == token {
				continue;
			}
			assert!(
				verify(&tampered, &key.verifying_key(), now).is_err(),
				"byte {index} changed and the token still verified"
			);
		}
	}

	#[test]
	fn malformed_tokens_are_unknown() {
		let now = Utc::now();
		let key = key(1);
		let token = sign(&key, &claims(now)).unwrap();
		let verifying = key.verifying_key();
		let (payload, signature) = token.rsplit_once('.').unwrap();
		for malformed in [
			String::new(),
			SIGNED_PREFIX.to_string(),
			token.trim_start_matches(SIGNED_PREFIX).to_string(),
			payload.to_string(),
			format!("{payload}."),
			format!("{payload}.{}", &signature[..signature.len() - 4]),
			format!("{token}.extra"),
			format!("{SIGNED_PREFIX}{}", "A".repeat(MAX_TOKEN_LEN)),
			format!("{SIGNED_PREFIX}not base64!.{signature}"),
		] {
			assert_eq!(
				verify(&malformed, &verifying, now),
				Err(GrantError::UnknownToken),
				"{malformed}"
			);
		}
	}

	#[test]
	fn claims_from_another_version_or_with_extra_fields_are_refused() {
		let now = Utc::now();
		let key = key(1);
		let mut future = claims(now);
		future.v = 2;
		assert_eq!(
			verify(&sign(&key, &future).unwrap(), &key.verifying_key(), now),
			Err(GrantError::UnknownToken)
		);

		let mut value = serde_json::to_value(claims(now)).unwrap();
		value["admin"] = serde_json::Value::Bool(true);
		let payload = serde_json::to_vec(&value).unwrap();
		let signature = key.sign(&signed_message(&payload));
		let token = format!(
			"{SIGNED_PREFIX}{}.{}",
			URL_SAFE_NO_PAD.encode(&payload),
			URL_SAFE_NO_PAD.encode(signature.to_bytes())
		);
		assert_eq!(
			verify(&token, &key.verifying_key(), now),
			Err(GrantError::UnknownToken)
		);
	}

	#[test]
	fn a_signature_made_without_the_domain_label_is_refused() {
		let now = Utc::now();
		let key = key(1);
		let payload = serde_json::to_vec(&claims(now)).unwrap();
		let signature = key.sign(&payload);
		let token = format!(
			"{SIGNED_PREFIX}{}.{}",
			URL_SAFE_NO_PAD.encode(&payload),
			URL_SAFE_NO_PAD.encode(signature.to_bytes())
		);
		assert_eq!(
			verify(&token, &key.verifying_key(), now),
			Err(GrantError::UnknownToken)
		);
	}
}
