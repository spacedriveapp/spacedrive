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
use crate::ops::grants::{authorize, GrantCap, GrantError, GrantScope};
use crate::service::mounts::tree::{BoundGrant, MountRoot};

/// How often a grant-bound mount checks that its grant is still active.
const GRANT_CHECK_INTERVAL: Duration = Duration::from_secs(5);

/// Why a mount could not be set up before the kernel was asked.
#[derive(Debug)]
pub(crate) enum Refusal {
	Mountpoint(String),
	Grant(GrantError),
	NoSource,
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
	let sources = context.volume_index().sources();
	let source = crate::ops::paths::reach::innermost_source(&sources, &scope.root)
		.ok_or(Refusal::NoSource)?;
	let grant = BoundGrant {
		token,
		root: MountRoot {
			name: String::new(),
			source_id: source.id,
			path: scope.root.clone(),
			remote: false,
		},
	};
	Ok((grant, scope))
}

/// Return once the grant is revoked, expires, or disappears. Each open
/// already checks the grant; the caller uses this to take the mount itself
/// down.
pub(crate) async fn grant_ended(context: Arc<CoreContext>, library_id: Uuid, grant_id: Uuid) {
	use crate::ops::grants::{store, GrantStatus};

	loop {
		tokio::time::sleep(GRANT_CHECK_INTERVAL).await;
		let Some(library) = context.libraries().await.get_library(library_id).await else {
			return;
		};
		match store::by_id(library.db().conn(), grant_id).await {
			Ok(Some(row)) if store::status(&row, Utc::now()) == GrantStatus::Active => continue,
			Ok(_) => return,
			Err(err) => warn!(grant = %grant_id, error = %err, "could not read a mounted grant"),
		}
	}
}
