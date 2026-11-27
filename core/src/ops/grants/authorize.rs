//! The one enforcement point for agent-facing surfaces.

use std::{path::Path, time::Instant};

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{debug, warn};
use uuid::Uuid;

use super::{
	audit::{self, AuditId, Decision, RequestMeta, TallyKind},
	bucket, scope, store, token, GrantCap, GrantError, GrantScope, GrantStatus,
};
use crate::{
	context::CoreContext,
	infra::db::entities::{agent_grant, source},
};

/// Decide whether `token` may perform `op` on `path` with capability `cap`.
///
/// This is the only check an agent-facing surface makes, and it makes it on
/// every request: the grant row is read each time, so a revocation or an
/// expiry refuses the next request with no cache to wait out. `path` is
/// relative to the granted directory, or absolute. On success the surface
/// must operate on [`GrantScope::path`], never on the path it was sent.
/// `op` is the surface's own name for the request and is only recorded.
///
/// Each open library is asked in turn, since a token names no library. Every
/// decision against a known grant lands in that library's audit table; an
/// allowed request whose audit row cannot be written is refused, so nothing
/// is served unrecorded. Surfaces that know more about the request, such as
/// its client address, call [`authorize_with`].
pub async fn authorize(
	context: &CoreContext,
	token: &str,
	op: &str,
	path: &Path,
	cap: GrantCap,
) -> Result<GrantScope, GrantError> {
	authorize_with(context, token, op, path, cap, &RequestMeta::default())
		.await
		.map(|(scope, _)| scope)
}

/// [`authorize`] for a surface that records what it knows of the request
/// and completes the audit row itself.
///
/// Returns the scope and the id of the request's audit row, which the
/// surface passes to [`audit::finish`] with the bytes, result and duration
/// before it sends the response.
///
/// A token that matches no grant is counted in every open library's
/// unknown-token tally for `meta.client`, one row per address per minute,
/// since nothing tells which library it was aimed at. A surface with no
/// client address only logs it. A count that cannot be written answers
/// [`GrantError::Internal`].
pub async fn authorize_with(
	context: &CoreContext,
	token: &str,
	op: &str,
	path: &Path,
	cap: GrantCap,
	meta: &RequestMeta,
) -> Result<(GrantScope, AuditId), GrantError> {
	let libraries = context.libraries().await.list().await;
	for library in &libraries {
		match authorize_in_with(
			context,
			library.db().conn(),
			library.id(),
			token,
			op,
			path,
			cap,
			meta,
		)
		.await
		{
			Err(GrantError::UnknownToken) => continue,
			decided => return decided,
		}
	}
	warn!(op, client = ?meta.client, "refused a request carrying an unknown grant token");
	if let Some(client) = meta.client {
		for library in &libraries {
			audit::tally(
				library.db().conn(),
				TallyKind::UnknownToken,
				&client.to_string(),
				None,
				Some(client),
			)
			.await
			.map_err(|e| GrantError::Internal(format!("audit: {e}")))?;
		}
	}
	Err(GrantError::UnknownToken)
}

/// [`authorize`] against one library's grants.
///
/// Answers [`GrantError::UnknownToken`] without recording anything when the
/// token is not one of this library's.
pub async fn authorize_in(
	context: &CoreContext,
	db: &DatabaseConnection,
	library_id: Uuid,
	token: &str,
	op: &str,
	path: &Path,
	cap: GrantCap,
) -> Result<GrantScope, GrantError> {
	authorize_in_with(
		context,
		db,
		library_id,
		token,
		op,
		path,
		cap,
		&RequestMeta::default(),
	)
	.await
	.map(|(scope, _)| scope)
}

async fn authorize_in_with(
	context: &CoreContext,
	db: &DatabaseConnection,
	library_id: Uuid,
	token: &str,
	op: &str,
	path: &Path,
	cap: GrantCap,
	meta: &RequestMeta,
) -> Result<(GrantScope, AuditId), GrantError> {
	let started = Instant::now();
	let hash = token::hash(token).ok_or(GrantError::UnknownToken)?;
	let grant = agent_grant::Entity::find()
		.filter(agent_grant::Column::TokenHash.eq(hash))
		.one(db)
		.await
		.map_err(|e| GrantError::Internal(format!("read grant: {e}")))?
		.ok_or(GrantError::UnknownToken)?;

	let decision = decide(context, db, library_id, &grant, path, cap, Utc::now()).await;
	let shown = path.display().to_string();
	let recorded = audit::record(
		db,
		Decision {
			grant: &grant,
			op,
			path: &shown,
			meta,
			outcome: decision.as_ref().map(drop),
			duration: decision.is_err().then(|| started.elapsed()),
		},
	)
	.await;
	match (decision, recorded) {
		(Ok(scope), Ok(row)) => {
			debug!(grant = %grant.uuid, op, path = %scope.path.display(), "grant authorized");
			Ok((scope, row))
		}
		(Ok(_), Err(e)) => {
			warn!(grant = %grant.uuid, error = %e, "refused a request that could not be audited");
			Err(GrantError::Internal(format!("audit: {e}")))
		}
		(Err(refusal), recorded) => {
			if let Err(e) = recorded {
				warn!(grant = %grant.uuid, error = %e, "could not audit a refusal");
			}
			debug!(grant = %grant.uuid, op, path = %shown, %refusal, "grant refused");
			Err(refusal)
		}
	}
}

async fn decide(
	context: &CoreContext,
	db: &DatabaseConnection,
	library_id: Uuid,
	grant: &agent_grant::Model,
	path: &Path,
	cap: GrantCap,
	now: DateTime<Utc>,
) -> Result<GrantScope, GrantError> {
	match store::status(grant, now) {
		GrantStatus::Revoked => return Err(GrantError::Revoked),
		GrantStatus::Expired => return Err(GrantError::Expired),
		GrantStatus::Active => {}
	}

	let source_present = source::Entity::find()
		.filter(source::Column::Uuid.eq(grant.source_id))
		.one(db)
		.await
		.map_err(|e| GrantError::Internal(format!("read source: {e}")))?
		.is_some();
	if !source_present {
		return Err(GrantError::SourceGone);
	}

	let held = GrantCap::parse(&grant.caps).unwrap_or(GrantCap::Read);
	if !held.allows(cap) {
		return Err(GrantError::ReadOnly);
	}

	let root = std::path::PathBuf::from(&grant.subtree);
	let resolved = match bucket::BucketUrl::parse(&root) {
		Some(url) => {
			let resolved = bucket::resolve(&url, path)?;
			if cap == GrantCap::ReadWrite {
				let backend = bucket::backend(context, &url).await?;
				bucket::writable(&url, backend.as_ref())?;
			}
			resolved
		}
		None => {
			let requested = path.to_path_buf();
			tokio::task::spawn_blocking(move || scope::resolve(&root, &requested))
				.await
				.map_err(|e| GrantError::Internal(format!("resolve: {e}")))??
		}
	};

	if cap == GrantCap::ReadWrite {
		// The spelling sent and the spelling it resolved to are both checked,
		// so neither a link named `notes.md` pointing at AGENTS.md nor an alias
		// spelling of a protected name gets through.
		if let Some(name) =
			scope::protected_component(path).or_else(|| scope::protected_component(&resolved.path))
		{
			return Err(GrantError::Protected(name));
		}
	}

	Ok(GrantScope {
		grant_id: grant.uuid,
		library_id,
		cap,
		root: resolved.root,
		path: resolved.path,
		relative: resolved.relative,
		expires_at: grant.expires_at,
	})
}
