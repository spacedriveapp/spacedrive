//! Reads and writes of the grant table shared by the ops.

use std::{path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use super::{audit, GrantCap, GrantInfo, GrantKind, GrantStatus, TokenFormat};
use crate::{
	context::CoreContext,
	infra::{
		api::SessionContext,
		db::entities::agent_grant,
		query::{QueryError, QueryResult},
	},
	library::Library,
};

/// Whether `row` can be used at `now`.
pub fn status(row: &agent_grant::Model, now: DateTime<Utc>) -> GrantStatus {
	if row.revoked_at.is_some() {
		GrantStatus::Revoked
	} else if now >= row.expires_at {
		GrantStatus::Expired
	} else {
		GrantStatus::Active
	}
}

/// The owner's view of a row.
pub fn info(row: agent_grant::Model, usage: audit::Usage) -> GrantInfo {
	GrantInfo {
		status: status(&row, Utc::now()),
		id: row.uuid,
		source_id: row.source_id,
		subtree: PathBuf::from(row.subtree),
		// An unknown value can only come from a newer build; read it as the
		// narrower capability.
		caps: GrantCap::parse(&row.caps).unwrap_or(GrantCap::Read),
		space_id: row.space_id,
		kind: GrantKind::parse(&row.kind),
		token_format: TokenFormat::parse(&row.token_format),
		label: row.label,
		created_by: row.created_by,
		created_at: row.created_at,
		expires_at: row.expires_at,
		revoked_at: row.revoked_at,
		last_used_at: usage.last_used_at,
		request_count: usage.requests,
	}
}

/// One grant by id.
pub async fn by_id(
	db: &DatabaseConnection,
	id: Uuid,
) -> Result<Option<agent_grant::Model>, sea_orm::DbErr> {
	agent_grant::Entity::find()
		.filter(agent_grant::Column::Uuid.eq(id))
		.one(db)
		.await
}

/// One grant by id with its last use, for an op's answer.
pub async fn info_by_id(db: &DatabaseConnection, id: Uuid) -> Result<GrantInfo, sea_orm::DbErr> {
	let row = by_id(db, id)
		.await?
		.ok_or_else(|| sea_orm::DbErr::RecordNotFound(format!("no grant {id}")))?;
	let usage = audit::usage(db).await?.remove(&id).unwrap_or_default();
	Ok(info(row, usage))
}

/// End one grant now. Revoking twice keeps the first revocation time.
pub async fn revoke(db: &DatabaseConnection, id: Uuid) -> Result<u64, sea_orm::DbErr> {
	Ok(agent_grant::Entity::update_many()
		.col_expr(
			agent_grant::Column::RevokedAt,
			sea_orm::sea_query::Expr::value(Utc::now()),
		)
		.filter(agent_grant::Column::Uuid.eq(id))
		.filter(agent_grant::Column::RevokedAt.is_null())
		.exec(db)
		.await?
		.rows_affected)
}

/// Record a revoked grant's revocation in its space's store, so every node
/// that holds the store refuses the grant. A grant made before spaces has no
/// space and nothing to record.
pub async fn publish_revocation(
	context: &CoreContext,
	row: &agent_grant::Model,
) -> Result<(), String> {
	if row.space_id.is_none() {
		return Ok(());
	}
	let index = context.volume_index();
	let root = index
		.source_root(row.source_id)
		.ok_or("the source holding this grant's space is not on this device")?;
	let store = index
		.store_for(&root)
		.await
		.ok_or("the source holding this grant's space has no store on this device")?;
	let stamp = crate::ops::spaces::store::stamp(context)?;
	store
		.db()
		.revoke_grant(&sd_store::GrantRevocation {
			grant_uuid: row.uuid,
			revoked_at: row.revoked_at.unwrap_or_else(Utc::now).to_rfc3339(),
			hlc: stamp.hlc,
			origin_device: stamp.device_uuid,
		})
		.await
		.map_err(|e| e.to_string())?;
	Ok(())
}

/// End every live grant on a source. Returns how many local grants ended.
///
/// The source's store also gets a revocation for every grant it asserts, so
/// a signed grant served from a copy of that store is refused there too.
/// Call it while the store is still open; a store not on this device is
/// skipped with a warning, since the source is being removed regardless.
pub async fn revoke_for_source(
	context: &CoreContext,
	db: &DatabaseConnection,
	source_id: Uuid,
) -> Result<u64, String> {
	let now = Utc::now();
	let ended = agent_grant::Entity::update_many()
		.col_expr(
			agent_grant::Column::RevokedAt,
			sea_orm::sea_query::Expr::value(now),
		)
		.filter(agent_grant::Column::SourceId.eq(source_id))
		.filter(agent_grant::Column::RevokedAt.is_null())
		.exec(db)
		.await
		.map_err(|e| e.to_string())?
		.rows_affected;
	if ended > 0 {
		tracing::info!(source = %source_id, ended, "grants revoked with their source");
	}

	let index = context.volume_index();
	let root = index.source_root(source_id);
	let store = match &root {
		Some(root) => index.store_for(root).await,
		None => None,
	};
	let Some(store) = store else {
		tracing::warn!(source = %source_id, "no store to record the source's grant revocations in");
		return Ok(ended);
	};
	let held = store
		.db()
		.grant_assertions()
		.await
		.map_err(|e| e.to_string())?;
	let stamp = crate::ops::spaces::store::stamp(context)?;
	for grant in held
		.grants
		.iter()
		.filter(|grant| !held.revocations.iter().any(|r| r.grant_uuid == grant.uuid))
	{
		store
			.db()
			.revoke_grant(&sd_store::GrantRevocation {
				grant_uuid: grant.uuid,
				revoked_at: now.to_rfc3339(),
				hlc: stamp.hlc.clone(),
				origin_device: stamp.device_uuid,
			})
			.await
			.map_err(|e| e.to_string())?;
	}
	Ok(ended)
}

/// The library a query's session names.
pub async fn session_library(
	context: &CoreContext,
	session: &SessionContext,
) -> QueryResult<Arc<Library>> {
	let library_id = session
		.current_library_id
		.ok_or_else(|| QueryError::Internal("No library in session".to_string()))?;
	context
		.libraries()
		.await
		.get_library(library_id)
		.await
		.ok_or_else(|| QueryError::Internal("Library not found".to_string()))
}
