//! The one enforcement point for agent-facing surfaces.

use std::{
	path::{Path, PathBuf},
	time::Instant,
};

use chrono::{DateTime, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{debug, warn};
use uuid::Uuid;

use super::{
	audit::{self, AuditId, Decision, RequestMeta, TallyKind},
	bucket, scope,
	verify::{self, GrantRecord},
	GrantCap, GrantError, GrantScope, GrantStatus,
};
use crate::{
	common::contain,
	context::CoreContext,
	domain::Space,
	infra::db::entities::source,
	library::Library,
	ops::spaces::store::{self as spaces, slash_path},
};

/// One request an agent-facing surface asks [`authorize`] about.
pub struct Access<'a> {
	pub token: &'a str,
	/// The surface's own name for the request. Only recorded.
	pub op: &'a str,
	/// The space the request names. None means the grant's own space.
	pub space: Option<Uuid>,
	/// Relative to the granted directory, or absolute. The grant subtree is
	/// the limit even when its space is larger.
	pub path: &'a Path,
	pub cap: GrantCap,
	/// The authority epoch the client wrote against. None for reads.
	pub epoch: Option<u64>,
	pub meta: RequestMeta,
}

impl<'a> Access<'a> {
	/// A request on the grant's own space with no epoch and nothing known of
	/// its client, which is what a local surface such as a mount sends.
	pub fn new(token: &'a str, op: &'a str, path: &'a Path, cap: GrantCap) -> Self {
		Self {
			token,
			op,
			space: None,
			path,
			cap,
			epoch: None,
			meta: RequestMeta::default(),
		}
	}
}

/// An allowed request: what it may touch and the audit row it completes.
#[derive(Debug, Clone)]
pub struct Authorized {
	pub scope: GrantScope,
	/// Passed to [`audit::finish`] with the bytes, result and duration
	/// before the response is sent.
	pub audit: AuditId,
}

/// Decide whether `access` may go ahead.
///
/// This is the only check an agent-facing surface makes, and it makes it on
/// every request. The token is verified each time, opaque by its stored hash
/// and signed by its issuer's key, and the grant's revocation is read each
/// time, so a revocation or an expiry refuses the next request with no cache
/// to wait out. On success the surface must operate on [`GrantScope::path`],
/// never on the path it was sent.
///
/// Each open library is asked in turn, since a token names no library.
///
/// Every decision against a known grant lands in its library's audit table;
/// an allowed request whose audit row cannot be written is refused, so
/// nothing is served unrecorded. A token that matches no grant is counted in
/// every open library's unknown-token tally for `meta.client`, one row per
/// address per minute, since nothing tells which library it was aimed at. A
/// surface with no client address only logs it. A count that cannot be
/// written answers [`GrantError::Internal`].
pub async fn authorize(
	context: &CoreContext,
	access: &Access<'_>,
) -> Result<Authorized, GrantError> {
	for library in context.libraries().await.list().await {
		match verify::verify(context, library.db().conn(), library.id(), access.token).await {
			Ok(grant) => return decide_recorded(context, &library, &grant, access).await,
			Err(GrantError::UnknownToken) => continue,
			Err(refusal) => return Err(refusal),
		}
	}
	refuse_unknown(context, access).await?;
	Err(GrantError::UnknownToken)
}

/// [`authorize`] for a surface that names no space and no epoch.
pub async fn authorize_with(
	context: &CoreContext,
	token: &str,
	op: &str,
	path: &Path,
	cap: GrantCap,
	meta: &RequestMeta,
) -> Result<(GrantScope, AuditId), GrantError> {
	let access = Access {
		meta: meta.clone(),
		..Access::new(token, op, path, cap)
	};
	authorize(context, &access)
		.await
		.map(|authorized| (authorized.scope, authorized.audit))
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
	let grant = verify::verify(context, db, library_id, token).await?;
	let library = context
		.libraries()
		.await
		.get_library(library_id)
		.await
		.ok_or(GrantError::UnknownToken)?;
	decide_recorded(
		context,
		&library,
		&grant,
		&Access::new(token, op, path, cap),
	)
	.await
	.map(|authorized| authorized.scope)
}

async fn refuse_unknown(context: &CoreContext, access: &Access<'_>) -> Result<(), GrantError> {
	let meta = &access.meta;
	warn!(op = access.op, client = ?meta.client, "refused a request carrying an unknown grant token");
	let Some(client) = meta.client else {
		return Ok(());
	};
	for library in context.libraries().await.list().await {
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
	Ok(())
}

async fn decide_recorded(
	context: &CoreContext,
	library: &Library,
	grant: &GrantRecord,
	access: &Access<'_>,
) -> Result<Authorized, GrantError> {
	let started = Instant::now();
	let db = library.db().conn();
	let op = access.op;
	let decision = decide(context, library, grant, access, Utc::now()).await;
	let shown = access.path.display().to_string();
	let recorded = audit::record(
		db,
		Decision {
			grant_id: grant.grant_id,
			label: grant.label.as_deref(),
			op,
			path: &shown,
			meta: &access.meta,
			outcome: decision.as_ref().map(drop),
			duration: decision.is_err().then(|| started.elapsed()),
		},
	)
	.await;
	match (decision, recorded) {
		(Ok(scope), Ok(audit)) => {
			debug!(grant = %grant.grant_id, space = %scope.space_id, op, path = %scope.path.display(), "grant authorized");
			Ok(Authorized { scope, audit })
		}
		(Ok(_), Err(e)) => {
			warn!(grant = %grant.grant_id, error = %e, "refused a request that could not be audited");
			Err(GrantError::Internal(format!("audit: {e}")))
		}
		(Err(refusal), recorded) => {
			if let Err(e) = recorded {
				warn!(grant = %grant.grant_id, error = %e, "could not audit a refusal");
			}
			debug!(grant = %grant.grant_id, op, path = %shown, %refusal, "grant refused");
			Err(refusal)
		}
	}
}

/// The granted directory relative to its source root, `/`-separated, when
/// the source is on this device.
///
/// A source is rooted at its volume's spelling of a path, which on macOS is
/// not always the canonical one a grant stores, so both are tried.
async fn grant_relative(context: &CoreContext, grant: &GrantRecord) -> Option<String> {
	let root = context.volume_index().source_root(grant.source_id)?;
	let subtree = grant.subtree.clone();
	let spelled = context
		.volume_manager
		.locate_path(&subtree)
		.await
		.map(|(_, spelled)| spelled);
	spelled
		.iter()
		.map(PathBuf::as_path)
		.chain(std::iter::once(subtree.as_path()))
		.find_map(|candidate| candidate.strip_prefix(&root).ok())
		.map(slash_path)
}

/// The space a grant lends from, checked against the one a request names.
///
/// A grant belongs to the innermost space of its source whose root holds
/// the granted directory. Only a grant outside every space is a space of its
/// own, whose id is the grant id. `relative` is the granted directory under
/// its source root, or None when the source is not on this device. A request
/// naming any other space is [`GrantError::UnknownSpace`].
fn space_for(
	spaces: &[Space],
	grant_id: Uuid,
	source_id: Uuid,
	relative: Option<&str>,
	requested: Option<Uuid>,
) -> Result<Uuid, GrantError> {
	let holds = |root: &str, relative: &str| {
		root.is_empty()
			|| relative
				.strip_prefix(root)
				.is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
	};
	let resolved = relative
		.and_then(|relative| {
			spaces
				.iter()
				.filter(|space| space.source_id == source_id && holds(&space.root, relative))
				.max_by_key(|space| space.root.len())
		})
		.map_or(grant_id, |space| space.id);
	named(resolved, requested)
}

/// `resolved`, unless the request names another space.
fn named(resolved: Uuid, requested: Option<Uuid>) -> Result<Uuid, GrantError> {
	match requested {
		Some(requested) if requested != resolved => Err(GrantError::UnknownSpace),
		_ => Ok(resolved),
	}
}

/// Refuse a write against an authority epoch the space has moved past.
///
/// Spaces store no epoch yet, so `current` is 0: a write naming 0 passes
/// and one naming anything else is stale. A request naming no epoch is not
/// checked.
fn fence(epoch: Option<u64>, current: u64) -> Result<(), GrantError> {
	match epoch {
		Some(epoch) if epoch != current => Err(GrantError::StaleEpoch { current }),
		_ => Ok(()),
	}
}

/// Confirms by handle what [`scope::resolve`] decided by path.
///
/// `resolve` follows links with `canonicalize`, which a link swapped in
/// afterwards defeats, so the deepest existing part of the resolved path is
/// opened again beneath the root, without reading it. An existing target is
/// that part; a create's is the parent it will be made in. Anything that
/// resolves outside the root, or that cannot be confirmed, is refused.
fn reachable(resolved: &scope::Resolved, requested: &Path) -> Result<(), GrantError> {
	for candidate in resolved.relative.ancestors() {
		if candidate.as_os_str().is_empty() {
			return Ok(());
		}
		match contain::reach_beneath(&resolved.root, candidate) {
			Ok(()) => return Ok(()),
			// A create names a parent that is missing or a file; the write
			// itself refuses the second.
			Err(error)
				if matches!(
					error.kind(),
					std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
				) =>
			{
				continue
			}
			Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {
				return Err(GrantError::InvalidPath(requested.display().to_string()));
			}
			Err(error) => {
				debug!(%error, "a resolved path did not open beneath its root");
				return Err(GrantError::Escape(requested.display().to_string()));
			}
		}
	}
	Ok(())
}

async fn decide(
	context: &CoreContext,
	library: &Library,
	grant: &GrantRecord,
	access: &Access<'_>,
	now: DateTime<Utc>,
) -> Result<GrantScope, GrantError> {
	let (path, cap) = (access.path, access.cap);
	match grant.status(now) {
		GrantStatus::Revoked => return Err(GrantError::Revoked),
		GrantStatus::Expired => return Err(GrantError::Expired),
		GrantStatus::Active => {}
	}

	let source_present = source::Entity::find()
		.filter(source::Column::Uuid.eq(grant.source_id))
		.one(library.db().conn())
		.await
		.map_err(|e| GrantError::Internal(format!("read source: {e}")))?
		.is_some();
	if !source_present {
		return Err(GrantError::SourceGone);
	}

	// A grant made inside a space names it. Older grants are placed by
	// their directory.
	let space_id = match grant.space_id {
		Some(stored) => named(stored, access.space)?,
		None => {
			let spaces = spaces::all(library, context)
				.await
				.map_err(|e| GrantError::Internal(format!("read spaces: {e}")))?;
			let relative = grant_relative(context, grant).await;
			space_for(
				&spaces,
				grant.grant_id,
				grant.source_id,
				relative.as_deref(),
				access.space,
			)?
		}
	};
	let authority_epoch = 0;
	fence(access.epoch, authority_epoch)?;

	if !grant.caps.allows(cap) {
		return Err(GrantError::ReadOnly);
	}

	let root = grant.subtree.clone();
	let resolved = match bucket::BucketUrl::parse(&root) {
		Some(url) => {
			let resolved = bucket::resolve(&url, path)?;
			if cap == GrantCap::ReadWrite {
				let backend = bucket::backend(&context.volume_manager, &url).await?;
				bucket::writable(&url, backend.as_ref())?;
			}
			resolved
		}
		None => {
			let requested = path.to_path_buf();
			tokio::task::spawn_blocking(move || {
				let resolved = scope::resolve(&root, &requested)?;
				reachable(&resolved, &requested)?;
				Ok::<_, GrantError>(resolved)
			})
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
		grant_id: grant.grant_id,
		actor_id: grant.actor_id.unwrap_or(grant.grant_id),
		library_id: library.id(),
		space_id,
		authority_epoch,
		cap,
		root: resolved.root,
		path: resolved.path,
		relative: resolved.relative,
		expires_at: grant.expires_at,
	})
}

#[cfg(test)]
mod tests {
	use chrono::Utc;

	use super::*;
	use crate::domain::SpaceKind;

	fn space(id: u128, source: Uuid, root: &str) -> Space {
		Space {
			id: Uuid::from_u128(id),
			name: format!("space {id}"),
			kind: SpaceKind::Folder,
			source_id: source,
			root: root.to_string(),
			path: None,
			content_indexing: true,
			created_at: Utc::now(),
		}
	}

	const GRANT: Uuid = Uuid::from_u128(1000);
	const SOURCE: Uuid = Uuid::from_u128(2000);

	#[test]
	fn a_grant_inside_a_space_resolves_to_that_space() {
		let spaces = [space(1, SOURCE, "projects")];
		assert_eq!(
			space_for(&spaces, GRANT, SOURCE, Some("projects/site"), None),
			Ok(Uuid::from_u128(1))
		);
		assert_eq!(
			space_for(&spaces, GRANT, SOURCE, Some("projects"), None),
			Ok(Uuid::from_u128(1))
		);
	}

	#[test]
	fn nested_spaces_resolve_to_the_innermost() {
		let spaces = [
			space(1, SOURCE, ""),
			space(2, SOURCE, "projects"),
			space(3, SOURCE, "projects/site"),
			space(4, SOURCE, "projects/site/docs"),
		];
		assert_eq!(
			space_for(&spaces, GRANT, SOURCE, Some("projects/site/src"), None),
			Ok(Uuid::from_u128(3))
		);
		assert_eq!(
			space_for(&spaces, GRANT, SOURCE, Some("photos"), None),
			Ok(Uuid::from_u128(1))
		);
	}

	#[test]
	fn a_grant_outside_every_space_is_its_own_space() {
		let other = Uuid::from_u128(3000);
		let spaces = [
			space(1, SOURCE, "projects"),
			space(2, other, ""),
			space(3, SOURCE, "projects/site"),
		];
		for relative in [Some("projects-old"), Some("proj"), Some(""), None] {
			assert_eq!(
				space_for(&spaces, GRANT, SOURCE, relative, None),
				Ok(GRANT),
				"{relative:?}"
			);
		}
		assert_eq!(
			space_for(&[], GRANT, SOURCE, Some("a"), Some(GRANT)),
			Ok(GRANT)
		);
	}

	#[test]
	fn a_request_naming_another_space_is_unknown() {
		let spaces = [space(1, SOURCE, "projects"), space(2, SOURCE, "photos")];
		let relative = Some("projects/site");
		assert_eq!(
			space_for(&spaces, GRANT, SOURCE, relative, Some(Uuid::from_u128(1))),
			Ok(Uuid::from_u128(1))
		);
		for wrong in [GRANT, Uuid::from_u128(2), Uuid::nil()] {
			assert_eq!(
				space_for(&spaces, GRANT, SOURCE, relative, Some(wrong)),
				Err(GrantError::UnknownSpace),
				"{wrong}"
			);
		}
	}

	#[test]
	fn a_stored_space_is_checked_against_the_named_one() {
		let stored = Uuid::from_u128(1);
		assert_eq!(named(stored, None), Ok(stored));
		assert_eq!(named(stored, Some(stored)), Ok(stored));
		assert_eq!(named(stored, Some(GRANT)), Err(GrantError::UnknownSpace));
	}

	#[test]
	fn only_the_current_epoch_passes() {
		assert_eq!(fence(None, 0), Ok(()));
		assert_eq!(fence(Some(0), 0), Ok(()));
		assert_eq!(
			fence(Some(1), 0),
			Err(GrantError::StaleEpoch { current: 0 })
		);
		assert_eq!(
			fence(Some(3), 4),
			Err(GrantError::StaleEpoch { current: 4 })
		);
	}
}
