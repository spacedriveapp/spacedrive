//! The verifier: what a presented token is a credential for.
//!
//! [`super::authorize`] asks this one step who a token belongs to, then
//! decides the request itself. Two formats reach it:
//!
//! - An opaque `sdg_` token is looked up by hash in this library's grant
//!   table, which only the minting device holds.
//! - A signed `sdg_s.` token names its space and its issuer. The space's
//!   store, on whichever node holds it, publishes the issuer's public key, so
//!   the signature is checked with no lookup on the device that minted it.
//!
//! Either way the revocation assertion in the space's store is read on every
//! call, so a revocation made on another device and merged into the store
//! refuses the next request here, with no cache to wait out.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use ed25519_dalek::VerifyingKey;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use super::{signed, store, token, GrantCap, GrantError, GrantKind, GrantStatus, TokenFormat};
use crate::{context::CoreContext, infra::db::entities::agent_grant};

/// A grant a token was verified as, before any request is checked against it.
#[derive(Debug, Clone)]
pub(crate) struct GrantRecord {
	pub grant_id: Uuid,
	pub source_id: Uuid,
	/// The granted directory on this node.
	pub subtree: PathBuf,
	pub caps: GrantCap,
	pub label: Option<String>,
	pub expires_at: DateTime<Utc>,
	pub revoked: bool,
	pub space_id: Option<Uuid>,
	pub actor_id: Option<Uuid>,
	pub kind: GrantKind,
	pub token_format: TokenFormat,
	/// The space's authority epoch the grant was minted under.
	pub epoch: i64,
}

impl GrantRecord {
	pub fn status(&self, now: DateTime<Utc>) -> GrantStatus {
		if self.revoked {
			GrantStatus::Revoked
		} else if now >= self.expires_at {
			GrantStatus::Expired
		} else {
			GrantStatus::Active
		}
	}
}

/// The grant `token` is a credential for in one library.
///
/// Answers [`GrantError::UnknownToken`] when the token is not one of this
/// library's, including a signed token whose signature does not check. An
/// expired or revoked grant is still returned, so its refusal is audited
/// against it.
pub(crate) async fn verify(
	context: &CoreContext,
	db: &DatabaseConnection,
	library_id: Uuid,
	token: &str,
) -> Result<GrantRecord, GrantError> {
	if signed::is_signed(token) {
		return verify_signed(context, db, library_id, token).await;
	}
	let hash = token::hash(token).ok_or(GrantError::UnknownToken)?;
	let row = agent_grant::Entity::find()
		.filter(agent_grant::Column::TokenHash.eq(hash))
		.one(db)
		.await
		.map_err(|e| GrantError::Internal(format!("read grant: {e}")))?
		.ok_or(GrantError::UnknownToken)?;
	let revoked = row.revoked_at.is_some()
		|| (row.space_id.is_some() && revoked_in_store(context, row.source_id, row.uuid).await?);
	Ok(GrantRecord {
		grant_id: row.uuid,
		source_id: row.source_id,
		subtree: PathBuf::from(&row.subtree),
		caps: GrantCap::parse(&row.caps).unwrap_or(GrantCap::Read),
		label: row.label,
		expires_at: row.expires_at,
		revoked,
		space_id: row.space_id,
		actor_id: row.actor_id,
		kind: GrantKind::parse(&row.kind),
		token_format: TokenFormat::parse(&row.token_format),
		epoch: row.epoch,
	})
}

/// [`verify`] in a library named by id, for the checks that run after
/// [`super::authorize`] already answered: a call, stream or mount polling
/// whether its grant ended, and the REST space listing. They read the same
/// credential the request was let in on, so a signed grant served from a
/// copied store is never mistaken for a missing one.
pub(crate) async fn verified(
	context: &CoreContext,
	library_id: Uuid,
	token: &str,
) -> Result<GrantRecord, GrantError> {
	let library = context
		.libraries()
		.await
		.get_library(library_id)
		.await
		.ok_or(GrantError::UnknownToken)?;
	verify(context, library.db().conn(), library_id, token).await
}

async fn verify_signed(
	context: &CoreContext,
	db: &DatabaseConnection,
	library_id: Uuid,
	token: &str,
) -> Result<GrantRecord, GrantError> {
	// Read only to find the store that can vouch for the token; nothing here
	// is trusted until the signature checks against that store's issuer key.
	let claimed = signed::unverified_claims(token)?;
	let index = context.volume_index();
	let mut holder = None;
	for source in index.sources_of(library_id) {
		let Some(store) = index.read_store(source.id).await else {
			continue;
		};
		let space = store
			.space(claimed.space_id)
			.await
			.map_err(|e| GrantError::Internal(format!("read space: {e}")))?;
		if let Some(space) = space.filter(|space| !space.deleted) {
			holder = Some((source.id, store, space));
			break;
		}
	}
	let (source_id, store, space) = holder.ok_or(GrantError::UnknownToken)?;
	let issuer = store
		.grant_issuer(claimed.iss)
		.await
		.map_err(|e| GrantError::Internal(format!("read grant issuer: {e}")))?
		.ok_or(GrantError::UnknownToken)?;
	let key = <[u8; 32]>::try_from(issuer.public_key.as_slice())
		.ok()
		.and_then(|bytes| VerifyingKey::from_bytes(&bytes).ok())
		.ok_or(GrantError::UnknownToken)?;
	let claims = signed::verify_signature(token, &key)?;

	let revoked_here = store::by_id(db, claims.grant_id)
		.await
		.map_err(|e| GrantError::Internal(format!("read grant: {e}")))?
		.is_some_and(|row| row.revoked_at.is_some());
	let revoked = revoked_here
		|| store
			.grant_revocation(claims.grant_id)
			.await
			.map_err(|e| GrantError::Internal(format!("read grant revocation: {e}")))?
			.is_some();
	let assertion = store
		.grant(claims.grant_id)
		.await
		.map_err(|e| GrantError::Internal(format!("read grant: {e}")))?;
	let source_root = index
		.source_root(source_id)
		.ok_or_else(|| GrantError::RootUnavailable("the space's source is not here".into()))?;
	let subtree = [space.root.as_str(), claims.subtree.as_str()]
		.into_iter()
		.filter(|part| !part.is_empty())
		.fold(source_root, |path, part| path.join(part));
	Ok(GrantRecord {
		grant_id: claims.grant_id,
		source_id,
		subtree,
		caps: claims.caps,
		label: assertion.as_ref().and_then(|row| row.label.clone()),
		expires_at: claims.expires_at(),
		revoked,
		space_id: Some(claims.space_id),
		actor_id: assertion.as_ref().and_then(|row| row.actor_uuid),
		kind: assertion
			.as_ref()
			.map_or(GrantKind::Task, |row| GrantKind::parse(&row.kind)),
		token_format: TokenFormat::Signed,
		epoch: claims.epoch,
	})
}

/// Whether the store of `source_id` holds a revocation of `grant`.
///
/// A source whose store is not on this node has nothing to say; the grant
/// table's own revocation still applies.
async fn revoked_in_store(
	context: &CoreContext,
	source_id: Uuid,
	grant: Uuid,
) -> Result<bool, GrantError> {
	let Some(store) = context.volume_index().read_store(source_id).await else {
		return Ok(false);
	};
	Ok(store
		.grant_revocation(grant)
		.await
		.map_err(|e| GrantError::Internal(format!("read grant revocation: {e}")))?
		.is_some())
}
