//! # Daemon mount helpers
//!
//! What the daemon's FUSE and NFS mounts share: the mount point check, the
//! grant a mount can be bound to, and the wait for that grant to end.
//!
//! Both frontends are held by the daemon on a user's request and serve the
//! same [`LibraryTree`], so a grant narrows and ends either mount the same
//! way. Only how the kernel is told to mount and unmount differs.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tracing::warn;
use uuid::Uuid;

use crate::context::CoreContext;
use crate::ops::grants::{authorize, create::owning_source, GrantCap, GrantError, GrantScope};
use crate::service::mounts::tree::{BoundGrant, MountRoot};

/// How often a grant-bound mount checks that its grant is still active.
const GRANT_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Why a mount could not be set up before the kernel was asked.
#[derive(Debug)]
pub(crate) enum Refusal {
	Mountpoint(String),
	Grant(GrantError),
	NoSource,
	Lookup(String),
}

/// `path`, absolute and canonical, once it is known to be an empty
/// directory. Blocking.
pub(crate) fn empty_dir(path: &Path) -> Result<PathBuf, Refusal> {
	let refuse = |reason: String| Refusal::Mountpoint(reason);
	if !path.is_absolute() {
		return Err(refuse(format!(
			"{} is not an absolute path",
			path.display()
		)));
	}
	let meta =
		std::fs::metadata(path).map_err(|err| refuse(format!("{}: {err}", path.display())))?;
	if !meta.is_dir() {
		return Err(refuse(format!("{} is not a directory", path.display())));
	}
	let mut entries =
		std::fs::read_dir(path).map_err(|err| refuse(format!("{}: {err}", path.display())))?;
	if entries.next().is_some() {
		return Err(refuse(format!("{} is not empty", path.display())));
	}
	std::fs::canonicalize(path).map_err(|err| refuse(format!("{}: {err}", path.display())))
}

/// Check `token` once and return the grant a mount binds to, with the scope
/// it lends. The mount then checks the token again on every listing and
/// open, recorded in the grant's audit log under `op`.
pub(crate) async fn bind_grant(
	context: &CoreContext,
	token: String,
	op: &str,
) -> Result<(BoundGrant, GrantScope), Refusal> {
	let scope = authorize(context, &token, op, Path::new(""), GrantCap::Read)
		.await
		.map_err(Refusal::Grant)?;
	// The grant keeps the directory's canonical spelling, which on macOS can
	// differ from the source's by a firmlink, and the tree answers only to
	// the source's spelling. A bucket grant has no directory to compare, so
	// its URL is matched by prefix.
	let sources = context.volume_index().sources();
	let (source, path) = match owning_source(context, scope.library_id, &scope.root)
		.await
		.map_err(Refusal::Lookup)?
	{
		Some(found) => found,
		None => crate::ops::paths::reach::innermost_source(&sources, &scope.root)
			.map(|source| (source.id, scope.root.clone()))
			.ok_or(Refusal::NoSource)?,
	};
	let grant = BoundGrant {
		token,
		root: MountRoot {
			name: String::new(),
			source_id: source,
			path,
			remote: false,
		},
	};
	Ok((grant, scope))
}

/// Return once the grant is revoked, expires, or disappears. Each open
/// already checks the grant; the caller uses this to take the mount itself
/// down. The token is verified rather than the grant looked up by id,
/// because a signed grant can be valid without a row in this library.
pub(crate) async fn grant_ended(
	context: Arc<CoreContext>,
	library_id: Uuid,
	grant_id: Uuid,
	token: String,
) {
	use crate::ops::grants::{verified, GrantStatus};

	loop {
		tokio::time::sleep(GRANT_CHECK_INTERVAL).await;
		match verified(&context, library_id, &token).await {
			Ok(grant) if grant.status(Utc::now()) == GrantStatus::Active => continue,
			Ok(_) | Err(GrantError::UnknownToken) => return,
			Err(err) => warn!(grant = %grant_id, error = %err, "could not read a mounted grant"),
		}
	}
}
