//! The one enforcement point for agent-facing surfaces.

use std::path::Path;

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{debug, warn};
use uuid::Uuid;

use super::{audit, bucket, scope, store, token, GrantCap, GrantError, GrantScope, GrantStatus};
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
/// is served unrecorded.
pub async fn authorize(
	context: &CoreContext,
	token: &str,
	op: &str,
	path: &Path,
	cap: GrantCap,
) -> Result<GrantScope, GrantError> {
	for library in context.libraries().await.list().await {
		match authorize_in(
			context,
			library.db().conn(),
			library.id(),
			token,
			op,
			path,
			cap,
		)
		.await
		{
			Err(GrantError::UnknownToken) => continue,
			decided => return decided,
		}
	}
	warn!(op, "refused a request carrying an unknown grant token");
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
	let hash = token::hash(token).ok_or(GrantError::UnknownToken)?;
	let grant = agent_grant::Entity::find()
		.filter(agent_grant::Column::TokenHash.eq(hash))
		.one(db)
		.await
		.map_err(|e| GrantError::Internal(format!("read grant: {e}")))?
		.ok_or(GrantError::UnknownToken)?;

	let decision = decide(context, db, library_id, &grant, path, cap, Utc::now()).await;
	let shown = path.display().to_string();
	let recorded = audit::record(db, grant.uuid, op, &shown, decision.as_ref().map(drop)).await;
	match (&decision, recorded) {
		(Ok(_), Err(e)) => {
			warn!(grant = %grant.uuid, error = %e, "refused a request that could not be audited");
			return Err(GrantError::Internal(format!("audit: {e}")));
		}
		(Err(_), Err(e)) => warn!(grant = %grant.uuid, error = %e, "could not audit a refusal"),
		_ => {}
	}
	match &decision {
		Ok(scope) => {
			debug!(grant = %grant.uuid, op, path = %scope.path.display(), "grant authorized")
		}
		Err(refusal) => debug!(grant = %grant.uuid, op, path = %shown, %refusal, "grant refused"),
	}
	decision
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
		actor_id: grant.actor_id.unwrap_or(grant.uuid),
		space_id: grant.space_id,
		library_id,
		cap,
		root: resolved.root,
		path: resolved.path,
		relative: resolved.relative,
		expires_at: grant.expires_at,
	})
}
