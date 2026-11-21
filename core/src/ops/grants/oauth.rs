//! The grant side of the MCP endpoint's OAuth flow.
//!
//! An OAuth access token is a grant token, so everything [`super::authorize`]
//! enforces applies to it unchanged and revoking the grant ends it. What
//! OAuth adds is a refresh token bound to the grant and to the client it was
//! issued to. Refreshing rotates both secrets on the same row: the old access
//! token stops working at once and the old refresh token is kept only as a
//! hash, so a replay of it marks it as leaked and revokes the grant.
//!
//! A refresh never extends a grant. The lifetime the owner picked on the
//! consent page, or set later with `grants.extend`, is the bound; once it has
//! passed, the client has to ask the owner again.

use chrono::{DateTime, Utc};
use sea_orm::{sea_query::Expr, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{info, warn};
use uuid::Uuid;

use super::{audit, store, token, GrantCap, GrantStatus};
use crate::{context::CoreContext, infra::db::entities::agent_grant};

/// The secrets a refresh issued, with what they allow.
#[derive(Debug)]
pub struct Rotated {
	pub grant_id: Uuid,
	pub access_token: String,
	pub refresh_token: String,
	pub caps: GrantCap,
	pub expires_at: DateTime<Utc>,
}

/// Why a refresh was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum RefreshError {
	/// The refresh token is unknown, belongs to another client, or its grant
	/// has ended. OAuth answers all of these with `invalid_grant`.
	Invalid,
	Internal(String),
}

/// Exchange `refresh_token` held by `client_id` for a new token pair.
pub async fn refresh(
	context: &CoreContext,
	client_id: &str,
	refresh_token: &str,
) -> Result<Rotated, RefreshError> {
	let hash = token::hash_refresh(refresh_token).ok_or(RefreshError::Invalid)?;
	for library in context.libraries().await.list().await {
		let db = library.db().conn();
		if let Some(row) = find(db, agent_grant::Column::RefreshHash, &hash).await? {
			return rotate(db, row, client_id, &hash).await;
		}
		if let Some(row) = find(db, agent_grant::Column::PreviousRefreshHash, &hash).await? {
			warn!(grant = %row.uuid, "a rotated refresh token was presented again; revoking the grant");
			store::revoke(db, row.uuid)
				.await
				.map_err(|e| RefreshError::Internal(e.to_string()))?;
			return Err(RefreshError::Invalid);
		}
	}
	Err(RefreshError::Invalid)
}

async fn find(
	db: &DatabaseConnection,
	column: agent_grant::Column,
	hash: &str,
) -> Result<Option<agent_grant::Model>, RefreshError> {
	agent_grant::Entity::find()
		.filter(column.eq(hash))
		.one(db)
		.await
		.map_err(|e| RefreshError::Internal(format!("read grant: {e}")))
}

async fn rotate(
	db: &DatabaseConnection,
	row: agent_grant::Model,
	client_id: &str,
	old_hash: &str,
) -> Result<Rotated, RefreshError> {
	if row.oauth_client.as_deref() != Some(client_id) {
		warn!(grant = %row.uuid, "refresh token presented by another client");
		return Err(RefreshError::Invalid);
	}
	if store::status(&row, Utc::now()) != GrantStatus::Active {
		return Err(RefreshError::Invalid);
	}
	let (access_token, access_hash) = token::mint();
	let (refresh_token, refresh_hash) = token::mint_refresh();
	// Matching on the old refresh hash and a live row makes two concurrent
	// refreshes of one token yield one winner, and never revives a grant
	// revoked between the read and this write.
	let updated = agent_grant::Entity::update_many()
		.col_expr(agent_grant::Column::TokenHash, Expr::value(access_hash))
		.col_expr(agent_grant::Column::RefreshHash, Expr::value(refresh_hash))
		.col_expr(
			agent_grant::Column::PreviousRefreshHash,
			Expr::value(old_hash.to_string()),
		)
		.filter(agent_grant::Column::Uuid.eq(row.uuid))
		.filter(agent_grant::Column::RefreshHash.eq(old_hash))
		.filter(agent_grant::Column::RevokedAt.is_null())
		.exec(db)
		.await
		.map_err(|e| RefreshError::Internal(format!("rotate grant tokens: {e}")))?;
	if updated.rows_affected == 0 {
		return Err(RefreshError::Invalid);
	}
	if let Err(e) = audit::record(db, row.uuid, "oauth.refresh", "", Ok(())).await {
		warn!(grant = %row.uuid, error = %e, "could not audit a token refresh");
	}
	info!(grant = %row.uuid, "grant tokens rotated by refresh");
	Ok(Rotated {
		grant_id: row.uuid,
		access_token,
		refresh_token,
		caps: GrantCap::parse(&row.caps).unwrap_or(GrantCap::Read),
		expires_at: row.expires_at,
	})
}

/// Revoke the grant an access or refresh token belongs to.
///
/// Answers whether a grant was found. Revocation clients are told nothing
/// either way, as RFC 7009 asks.
pub async fn revoke_token(context: &CoreContext, presented: &str) -> Result<bool, String> {
	let (column, hash) = match (token::hash(presented), token::hash_refresh(presented)) {
		(Some(hash), _) => (agent_grant::Column::TokenHash, hash),
		(None, Some(hash)) => (agent_grant::Column::RefreshHash, hash),
		(None, None) => return Ok(false),
	};
	for library in context.libraries().await.list().await {
		let db = library.db().conn();
		let row = find(db, column, &hash)
			.await
			.map_err(|e| format!("{e:?}"))?;
		if let Some(row) = row {
			store::revoke(db, row.uuid)
				.await
				.map_err(|e| e.to_string())?;
			info!(grant = %row.uuid, "grant revoked by its OAuth client");
			return Ok(true);
		}
	}
	Ok(false)
}
