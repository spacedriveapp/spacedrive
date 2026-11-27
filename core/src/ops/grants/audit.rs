//! The audit trail of grant-carrying requests.
//!
//! Every request [`super::authorize`] decides against a known grant is
//! recorded, allowed or refused, before the surface answers it. A row holds
//! the grant and its label, the client address, the MCP session, the
//! operation, the path as the agent sent it, the grep or glob pattern, the
//! bytes returned, the result and the duration. It never holds the token, file
//! contents or match text.
//!
//! The decision is written when it is made, and a surface that serves an
//! allowed request completes the row with [`finish`] once it has the response
//! body and before it sends it. Either write failing makes the request answer
//! as unavailable, so nothing is served unrecorded.
//!
//! Requests that a flood could repeat without limit are counted instead of
//! recorded one by one: unknown tokens and requests from a locked-out address
//! per client address per minute, rate-limited requests per grant per minute.
//! See [`tally`].
//!
//! Retention is [`RETENTION_DAYS`] days or [`RETENTION_ROWS`] rows, whichever
//! comes first. [`trim`] runs on about one write in [`TRIM_EVERY`] to each
//! library rather than on every write, so a table can briefly hold rows over
//! the cap.

use std::{collections::HashMap, net::IpAddr, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use sea_orm::{
	sea_query::{Expr, OnConflict},
	ActiveValue::Set,
	ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
	QuerySelect, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use tracing::warn;
use uuid::Uuid;

use super::GrantError;
use crate::{
	context::CoreContext,
	infra::{
		db::entities::{agent_grant, grant_audit, grant_audit_tally},
		query::{LibraryQuery, QueryError, QueryResult},
	},
};

/// The most rows each audit table keeps.
pub const RETENTION_ROWS: i64 = 1_000_000;
/// The age past which audit rows are removed.
pub const RETENTION_DAYS: i64 = 30;
/// Writes to one library between two retention passes, on average.
pub const TRIM_EVERY: u64 = 1000;
/// How much of a grep or glob pattern a row keeps.
pub const PATTERN_KEPT_BYTES: usize = 256;

/// The id of an audit row, for [`finish`].
pub type AuditId = i32;

/// What a surface knows about a request beyond the token, op and path.
///
/// Surfaces that are not reached over the network, such as a mount, leave
/// it empty.
#[derive(Debug, Clone, Default)]
pub struct RequestMeta {
	pub client: Option<IpAddr>,
	pub session: Option<String>,
	/// A grep or glob pattern. Only its first [`PATTERN_KEPT_BYTES`] bytes
	/// are recorded.
	pub pattern: Option<String>,
}

/// How a recorded request ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
	Ok,
	/// Refused by the grant or the endpoint.
	Denied,
	/// Refused by a per-grant limit.
	Limited,
	/// The request failed, or could not be decided.
	Error,
}

impl AuditResult {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::Ok => "ok",
			Self::Denied => "denied",
			Self::Limited => "limited",
			Self::Error => "error",
		}
	}

	pub fn parse(value: &str) -> Option<Self> {
		match value {
			"ok" => Some(Self::Ok),
			"denied" => Some(Self::Denied),
			"limited" => Some(Self::Limited),
			"error" => Some(Self::Error),
			_ => None,
		}
	}
}

/// What a tally row counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TallyKind {
	/// Requests whose token matched no grant, per client address.
	UnknownToken,
	/// Requests refused because their client address is locked out after
	/// failed authentications, per client address.
	LockedOut,
	/// Requests over a grant's request rate, per grant.
	RateLimited,
}

impl TallyKind {
	pub fn as_str(self) -> &'static str {
		match self {
			Self::UnknownToken => "unknown_token",
			Self::LockedOut => "locked_out",
			Self::RateLimited => "rate_limited",
		}
	}

	pub fn parse(value: &str) -> Option<Self> {
		match value {
			"unknown_token" => Some(Self::UnknownToken),
			"locked_out" => Some(Self::LockedOut),
			"rate_limited" => Some(Self::RateLimited),
			_ => None,
		}
	}
}

/// One decision of [`super::authorize`], to record.
pub(crate) struct Decision<'a> {
	pub grant: &'a agent_grant::Model,
	pub op: &'a str,
	pub path: &'a str,
	pub meta: &'a RequestMeta,
	pub outcome: Result<(), &'a GrantError>,
	/// Set for a refusal, which is complete when recorded. An allowed request
	/// gets its duration from [`finish`].
	pub duration: Option<Duration>,
}

/// Record one decision and return its row id.
///
/// An allowed request is recorded as `ok` so surfaces that never call
/// [`finish`] still read correctly; the ones that do overwrite it.
pub(crate) async fn record(
	db: &DatabaseConnection,
	decision: Decision<'_>,
) -> Result<AuditId, DbErr> {
	let Decision {
		grant,
		op,
		path,
		meta,
		outcome,
		duration,
	} = decision;
	let result = match outcome {
		Ok(()) => AuditResult::Ok,
		Err(GrantError::Internal(_)) => AuditResult::Error,
		Err(_) => AuditResult::Denied,
	};
	let inserted = grant_audit::Entity::insert(grant_audit::ActiveModel {
		grant_id: Set(grant.uuid),
		op: Set(op.to_string()),
		path: Set(path.to_string()),
		allowed: Set(outcome.is_ok()),
		reason: Set(outcome.err().map(ToString::to_string)),
		at: Set(Utc::now()),
		grant_label: Set(grant.label.clone()),
		client: Set(meta.client.map(|ip| ip.to_string())),
		session: Set(meta.session.clone()),
		pattern: Set(meta.pattern.as_deref().map(|p| clip(p).to_string())),
		bytes: Set(None),
		result: Set(Some(result.as_str().to_string())),
		duration_ms: Set(duration.map(millis)),
		..Default::default()
	})
	.exec(db)
	.await?;
	trim_when_due(db).await;
	Ok(inserted.last_insert_id)
}

/// Complete the row of an allowed request with how it ended.
///
/// Call it after the response body exists and before it is sent. A row that
/// is gone counts as a failed write, so the caller refuses to answer.
pub async fn finish(
	db: &impl ConnectionTrait,
	row: AuditId,
	bytes: Option<u64>,
	result: AuditResult,
	duration: Duration,
) -> Result<(), DbErr> {
	let updated = grant_audit::Entity::update_many()
		.col_expr(
			grant_audit::Column::Bytes,
			Expr::value(bytes.map(|b| i64::try_from(b).unwrap_or(i64::MAX))),
		)
		.col_expr(grant_audit::Column::Result, Expr::value(result.as_str()))
		.col_expr(
			grant_audit::Column::DurationMs,
			Expr::value(millis(duration)),
		)
		.filter(grant_audit::Column::Id.eq(row))
		.exec(db)
		.await?;
	if updated.rows_affected == 0 {
		return Err(DbErr::RecordNotUpdated);
	}
	Ok(())
}

/// Count one request in this minute's tally row for `kind` and `subject`.
pub async fn tally(
	db: &impl ConnectionTrait,
	kind: TallyKind,
	subject: &str,
	grant_id: Option<Uuid>,
	client: Option<IpAddr>,
) -> Result<(), DbErr> {
	let now = Utc::now();
	let start = now.timestamp() - now.timestamp().rem_euclid(60);
	let minute = DateTime::from_timestamp(start, 0).unwrap_or(now);
	grant_audit_tally::Entity::insert(grant_audit_tally::ActiveModel {
		kind: Set(kind.as_str().to_string()),
		subject: Set(subject.to_string()),
		grant_id: Set(grant_id),
		client: Set(client.map(|ip| ip.to_string())),
		minute: Set(minute),
		count: Set(1),
		last_at: Set(now),
		..Default::default()
	})
	.on_conflict(
		OnConflict::columns([
			grant_audit_tally::Column::Kind,
			grant_audit_tally::Column::Subject,
			grant_audit_tally::Column::Minute,
		])
		.value(
			grant_audit_tally::Column::Count,
			Expr::col(grant_audit_tally::Column::Count).add(1),
		)
		.update_columns([
			grant_audit_tally::Column::LastAt,
			grant_audit_tally::Column::Client,
		])
		.to_owned(),
	)
	.exec_without_returning(db)
	.await?;
	trim_when_due(db).await;
	Ok(())
}

/// Turn the row of a request that went over its grant's rate into a count in
/// that grant's tally, so a client hammering the limit adds no rows.
pub async fn fold_rate_limited(
	db: &DatabaseConnection,
	row: AuditId,
	grant_id: Uuid,
	client: Option<IpAddr>,
) -> Result<(), DbErr> {
	let txn = db.begin().await?;
	grant_audit::Entity::delete_by_id(row).exec(&txn).await?;
	tally(
		&txn,
		TallyKind::RateLimited,
		&grant_id.to_string(),
		Some(grant_id),
		client,
	)
	.await?;
	txn.commit().await
}

/// Remove rows older than `max_age` and all but the newest `max_rows` of
/// each audit table.
///
/// Ids have gaps, from folded rate-limited rows and from tally upserts, so the
/// cutoff is the id of the newest row past the cap rather than an offset from
/// the newest id.
pub async fn trim(
	db: &impl ConnectionTrait,
	max_rows: i64,
	max_age: chrono::Duration,
) -> Result<(), DbErr> {
	let cutoff = Utc::now() - max_age;
	grant_audit::Entity::delete_many()
		.filter(grant_audit::Column::At.lt(cutoff))
		.exec(db)
		.await?;
	grant_audit_tally::Entity::delete_many()
		.filter(grant_audit_tally::Column::Minute.lt(cutoff))
		.exec(db)
		.await?;

	let kept = u64::try_from(max_rows).unwrap_or(0);
	let cutoff: Option<i32> = grant_audit::Entity::find()
		.select_only()
		.column(grant_audit::Column::Id)
		.order_by_desc(grant_audit::Column::Id)
		.offset(kept)
		.limit(1)
		.into_tuple()
		.one(db)
		.await?;
	if let Some(cutoff) = cutoff {
		grant_audit::Entity::delete_many()
			.filter(grant_audit::Column::Id.lte(cutoff))
			.exec(db)
			.await?;
	}
	let cutoff: Option<i32> = grant_audit_tally::Entity::find()
		.select_only()
		.column(grant_audit_tally::Column::Id)
		.order_by_desc(grant_audit_tally::Column::Id)
		.offset(kept)
		.limit(1)
		.into_tuple()
		.one(db)
		.await?;
	if let Some(cutoff) = cutoff {
		grant_audit_tally::Entity::delete_many()
			.filter(grant_audit_tally::Column::Id.lte(cutoff))
			.exec(db)
			.await?;
	}
	Ok(())
}

async fn trim_when_due(db: &impl ConnectionTrait) {
	// A draw per write rather than a shared counter, so each library gets a
	// pass in proportion to its own writes, whichever libraries the traffic
	// alternates between.
	if rand::random::<u64>() % TRIM_EVERY != 0 {
		return;
	}
	// The write it follows already landed, so a failed pass only delays
	// retention to the next one.
	if let Err(e) = trim(db, RETENTION_ROWS, chrono::Duration::days(RETENTION_DAYS)).await {
		warn!(error = %e, "could not trim the grant audit");
	}
}

/// The longest prefix of `pattern` within [`PATTERN_KEPT_BYTES`] that ends on
/// a character boundary.
fn clip(pattern: &str) -> &str {
	let mut end = pattern.len().min(PATTERN_KEPT_BYTES);
	while !pattern.is_char_boundary(end) {
		end -= 1;
	}
	&pattern[..end]
}

fn millis(duration: Duration) -> i64 {
	i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

/// One grant's use, from the audit within its retention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
	/// When the grant last authorized a request.
	pub last_used_at: Option<DateTime<Utc>>,
	/// Requests recorded against the grant, rate-limited ones included.
	pub requests: u64,
}

/// Each grant's [`Usage`].
pub async fn usage(db: &impl ConnectionTrait) -> Result<HashMap<Uuid, Usage>, DbErr> {
	let mut usage: HashMap<Uuid, Usage> = HashMap::new();
	let rows: Vec<(Uuid, i64, Option<DateTime<Utc>>)> = grant_audit::Entity::find()
		.select_only()
		.column(grant_audit::Column::GrantId)
		.column_as(Expr::col(grant_audit::Column::Id).count(), "requests")
		.column_as(
			Expr::cust("MAX(CASE WHEN allowed THEN at END)"),
			"last_used_at",
		)
		.group_by(grant_audit::Column::GrantId)
		.into_tuple()
		.all(db)
		.await?;
	for (grant, requests, last_used_at) in rows {
		let entry = usage.entry(grant).or_default();
		entry.requests += u64::try_from(requests).unwrap_or(0);
		entry.last_used_at = last_used_at;
	}
	let limited: Vec<(Option<Uuid>, Option<i64>)> = grant_audit_tally::Entity::find()
		.select_only()
		.column(grant_audit_tally::Column::GrantId)
		.column_as(
			Expr::col(grant_audit_tally::Column::Count).sum(),
			"requests",
		)
		.filter(grant_audit_tally::Column::Kind.eq(TallyKind::RateLimited.as_str()))
		.group_by(grant_audit_tally::Column::GrantId)
		.into_tuple()
		.all(db)
		.await?;
	for (grant, requests) in limited {
		let Some(grant) = grant else { continue };
		usage.entry(grant).or_default().requests +=
			u64::try_from(requests.unwrap_or(0)).unwrap_or(0);
	}
	Ok(usage)
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantAuditInput {
	/// Only this grant's requests. Every grant's, and the unknown-token
	/// tallies, when absent.
	pub grant_id: Option<Uuid>,
	/// At most this many entries and this many tallies, newest first.
	/// Defaults to 100.
	pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantAuditEntry {
	pub grant_id: Uuid,
	pub grant_label: Option<String>,
	pub op: String,
	pub path: String,
	pub allowed: bool,
	pub reason: Option<String>,
	pub at: DateTime<Utc>,
	pub client: Option<String>,
	pub session: Option<String>,
	pub pattern: Option<String>,
	pub bytes: Option<u64>,
	/// Absent on rows recorded before results were kept.
	pub result: Option<AuditResult>,
	pub duration_ms: Option<u64>,
}

/// Requests of one kind from one subject in one minute.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantAuditTally {
	pub kind: TallyKind,
	/// The client address, or the grant id for rate-limited requests.
	pub subject: String,
	pub grant_id: Option<Uuid>,
	/// The client address of the latest request counted.
	pub client: Option<String>,
	pub minute: DateTime<Utc>,
	pub count: u64,
	pub last_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct GrantAuditOutput {
	pub entries: Vec<GrantAuditEntry>,
	pub tallies: Vec<GrantAuditTally>,
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
		let db = library.db().conn();
		let limit = u64::from(self.input.limit.unwrap_or(100));
		let internal = |e: DbErr| QueryError::Internal(format!("read the grant audit: {e}"));
		let to_u64 = |v: Option<i64>| v.and_then(|v| u64::try_from(v).ok());

		let mut query = grant_audit::Entity::find().order_by_desc(grant_audit::Column::Id);
		if let Some(grant_id) = self.input.grant_id {
			query = query.filter(grant_audit::Column::GrantId.eq(grant_id));
		}
		let entries = query
			.limit(limit)
			.all(db)
			.await
			.map_err(internal)?
			.into_iter()
			.map(|row| GrantAuditEntry {
				grant_id: row.grant_id,
				grant_label: row.grant_label,
				op: row.op,
				path: row.path,
				allowed: row.allowed,
				reason: row.reason,
				at: row.at,
				client: row.client,
				session: row.session,
				pattern: row.pattern,
				bytes: to_u64(row.bytes),
				result: row.result.as_deref().and_then(AuditResult::parse),
				duration_ms: to_u64(row.duration_ms),
			})
			.collect();

		let mut query =
			grant_audit_tally::Entity::find().order_by_desc(grant_audit_tally::Column::LastAt);
		if let Some(grant_id) = self.input.grant_id {
			query = query.filter(grant_audit_tally::Column::GrantId.eq(grant_id));
		}
		let tallies = query
			.limit(limit)
			.all(db)
			.await
			.map_err(internal)?
			.into_iter()
			.filter_map(|row| {
				Some(GrantAuditTally {
					kind: TallyKind::parse(&row.kind)?,
					subject: row.subject,
					grant_id: row.grant_id,
					client: row.client,
					minute: row.minute,
					count: u64::try_from(row.count).unwrap_or(0),
					last_at: row.last_at,
				})
			})
			.collect();
		Ok(GrantAuditOutput { entries, tallies })
	}
}

crate::register_library_query!(GrantAuditQuery, "grants.audit");

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn patterns_are_clipped_on_a_character_boundary() {
		assert_eq!(clip("short"), "short");
		let long = "a".repeat(300);
		assert_eq!(clip(&long).len(), PATTERN_KEPT_BYTES);
		let multibyte = format!("{}é", "a".repeat(PATTERN_KEPT_BYTES - 1));
		assert_eq!(clip(&multibyte).len(), PATTERN_KEPT_BYTES - 1);
	}
}
