//! Reads and writes of the grant table shared by the ops.

use std::{path::PathBuf, sync::Arc};

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use super::{audit, GrantCap, GrantInfo, GrantStatus};
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

/// End every live grant on a source. Returns how many were ended.
pub async fn revoke_for_source(
	db: &DatabaseConnection,
	source_id: Uuid,
) -> Result<u64, sea_orm::DbErr> {
	let ended = agent_grant::Entity::update_many()
		.col_expr(
			agent_grant::Column::RevokedAt,
			sea_orm::sea_query::Expr::value(Utc::now()),
		)
		.filter(agent_grant::Column::SourceId.eq(source_id))
		.filter(agent_grant::Column::RevokedAt.is_null())
		.exec(db)
		.await?
		.rows_affected;
	if ended > 0 {
		tracing::info!(source = %source_id, ended, "grants revoked with their source");
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
