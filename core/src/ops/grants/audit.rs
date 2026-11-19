//! The audit trail of grant-carrying requests.
//!
//! Every request [`super::authorize`] decides against a known grant is
//! recorded, allowed or refused, with the grant, the operation, the path as
//! the agent sent it, and the time. The table keeps the newest
//! [`AUDIT_CAPACITY`] rows and trims the rest on each write, so it is a
//! bounded ring rather than a log that grows with use.
//!
//! A request whose token matches no grant has no grant and no library to
//! record it against, so it is logged through `tracing` instead.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::{
	sea_query::Expr, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
	EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

use super::GrantError;
use crate::{
	context::CoreContext,
	infra::{
		db::entities::grant_audit,
		query::{LibraryQuery, QueryError, QueryResult},
	},
};

/// How many audit rows a library keeps.
pub const AUDIT_CAPACITY: i32 = 10_000;

/// Record one decision and trim the table to [`AUDIT_CAPACITY`].
pub async fn record(
	db: &DatabaseConnection,
	grant_id: Uuid,
	op: &str,
	path: &str,
	outcome: Result<(), &GrantError>,
) -> Result<(), sea_orm::DbErr> {
	let inserted = grant_audit::Entity::insert(grant_audit::ActiveModel {
		grant_id: Set(grant_id),
		op: Set(op.to_string()),
		path: Set(path.to_string()),
		allowed: Set(outcome.is_ok()),
		reason: Set(outcome.err().map(ToString::to_string)),
		at: Set(Utc::now()),
		..Default::default()
	})
	.exec(db)
	.await?;

	let oldest_kept = inserted.last_insert_id - AUDIT_CAPACITY;
	if oldest_kept > 0 {
		grant_audit::Entity::delete_many()
			.filter(grant_audit::Column::Id.lte(oldest_kept))
			.exec(db)
			.await?;
	}
	Ok(())
}

/// When each grant last authorized a request.
pub async fn last_used(
	db: &impl ConnectionTrait,
) -> Result<std::collections::HashMap<Uuid, DateTime<Utc>>, sea_orm::DbErr> {
	let rows: Vec<(Uuid, Option<DateTime<Utc>>)> = grant_audit::Entity::find()
		.select_only()
		.column(grant_audit::Column::GrantId)
		.column_as(Expr::col(grant_audit::Column::At).max(), "at")
		.filter(grant_audit::Column::Allowed.eq(true))
		.group_by(grant_audit::Column::GrantId)
		.into_tuple()
		.all(db)
		.await?;
	Ok(rows
		.into_iter()
		.filter_map(|(id, at)| Some((id, at?)))
		.collect())
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantAuditInput {
	/// Only this grant's requests. Every grant's when absent.
	pub grant_id: Option<Uuid>,
	/// At most this many entries, newest first. Defaults to 100.
	pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantAuditEntry {
	pub grant_id: Uuid,
	pub op: String,
	pub path: String,
	pub allowed: bool,
	pub reason: Option<String>,
	pub at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantAuditOutput {
	pub entries: Vec<GrantAuditEntry>,
}

/// Read the audit trail, newest first.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantAuditQuery {
	input: GrantAuditInput,
}

impl LibraryQuery for GrantAuditQuery {
	type Input = GrantAuditInput;
	type Output = GrantAuditOutput;

	fn from_input(input: Self::Input) -> QueryResult<Self> {
		Ok(Self { input })
	}

	async fn execute(
		self,
		context: Arc<CoreContext>,
		session: crate::infra::api::SessionContext,
	) -> QueryResult<Self::Output> {
		let library = super::store::session_library(&context, &session).await?;
		let mut query = grant_audit::Entity::find().order_by_desc(grant_audit::Column::Id);
		if let Some(grant_id) = self.input.grant_id {
			query = query.filter(grant_audit::Column::GrantId.eq(grant_id));
		}
		let entries = query
			.limit(u64::from(self.input.limit.unwrap_or(100)))
			.all(library.db().conn())
			.await
			.map_err(|e| QueryError::Internal(format!("read the grant audit: {e}")))?
			.into_iter()
			.map(|row| GrantAuditEntry {
				grant_id: row.grant_id,
				op: row.op,
				path: row.path,
				allowed: row.allowed,
				reason: row.reason,
				at: row.at,
			})
			.collect();
		Ok(GrantAuditOutput { entries })
	}
}

crate::register_library_query!(GrantAuditQuery, "grants.audit");
